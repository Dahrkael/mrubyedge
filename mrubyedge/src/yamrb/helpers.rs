use std::cell::RefCell;
use std::rc::Rc;

use crate::Error;

use super::{
    optable::push_callinfo,
    value::{
        FastOp, RClass, RFn, RHashMap, RModule, RObject, RProc, RValue, Value, intern_symbol,
        resolve_method,
    },
    vm::{CallerLabel, CallerReceiver, VM, arg_buf, value_args},
};

fn call_block(
    vm: &mut VM,
    block: RProc,
    recv: Value,
    args: &[Value],
    method_info: Option<(u32, Rc<RModule>)>,
    return_register: usize,
) -> Result<Value, Error> {
    let (method_id, method_owner) = match method_info {
        Some((id, owner)) => (id, Some(owner)),
        None => (intern_symbol("<block>"), None),
    };

    // the callee gets its own register window above the
    // caller's frame. Sharing the window made reentrant funcalls issued from
    // native code clobber in-flight caller registers.
    let saved_offset = vm.current_regs_offset;
    let caller_frame_size = vm.current_irep.nregs;
    // unbounded recursion raises SystemStackError instead of
    // overflowing the register array.
    vm.check_frame_window(caller_frame_size, 32)?;
    vm.current_regs_offset = saved_offset + caller_frame_size;

    // The funcall frame keeps its callinfo live while the callee runs so
    // super and op_enter can read the method name/owner; the is_funcall flag
    // makes op_return preempt back to this native caller. Restore from the
    // snapshot the callinfo captured at push.
    let caller_ci = vm.current_callinfo.clone();
    push_callinfo(
        vm,
        method_id,
        args.len(),
        method_owner,
        return_register,
        true,
    );
    let funcall_ci = vm.current_callinfo.clone().expect("callinfo just pushed");

    // Keep the state before the call inside the new window.
    let prev_self = vm.swap_reg_value(0, recv);

    let mut prev_args = Vec::with_capacity(args.len());
    for (i, arg) in args.iter().enumerate() {
        let old = vm.swap_reg_value(i + 1, arg.clone());
        prev_args.push(old);
    }

    // The callee's block local must be nil when no block is passed
    // (do_op_send does the same on the send path). Without it, methods that
    // read their block local — e.g. super forwarding the block — hit an
    // unassigned register.
    vm.set_reg_value(args.len() + 1, Value::Nil);

    vm.pc.set(0);
    vm.current_irep = block
        .irep
        .as_ref()
        .ok_or_else(|| Error::RuntimeError("No IREP".to_string()))?
        .clone();
    // the callee's own environ becomes the active upper env,
    // but the caller's must be restored on return. Deriving the restore from
    // the callee's environ chain (old behavior) clobbered vm.upper to None
    // after native->Ruby calls (e.g. Array#delete through rb_eq), corrupting
    // the upvar chain of enclosing blocks.
    let prev_upper = vm.upper.take();
    vm.upper = block.environ;

    let res = vm.run_internal();

    vm.current_regs()[0] = prev_self;
    for (i, prev_arg) in prev_args.into_iter().enumerate() {
        vm.current_regs()[i + 1] = prev_arg;
    }

    vm.current_callinfo = caller_ci;
    vm.current_irep = funcall_ci.pc_irep.clone();
    vm.pc.set(funcall_ci.pc);
    vm.current_regs_offset = saved_offset;
    vm.target_class = funcall_ci.target_class.clone();
    vm.upper = prev_upper;

    match res {
        Ok(v) => Ok(v),
        Err(e) => {
            let err = if let Some(e) = e.downcast_ref::<Error>() {
                e.clone()
            } else {
                // {:?} on foreign errors can walk cyclic VM
                // graphs (class -> module -> procs -> proc) and overflow.
                let _ = e;
                Error::RuntimeError("non-mrubyedge error escaped a block".to_string())
            };
            Err(err)
        }
    }
}

/// Calls a Ruby block (Proc) with the given receiver and arguments.
///
/// # Arguments
///
/// * `vm` - The virtual machine instance
/// * `block` - The block object to call (must be a Proc)
/// * `recv` - Optional receiver object. If None, uses the block's self
/// * `args` - Array of arguments to pass to the block
///
/// # Returns
///
/// Returns the result of the block execution or an error if the call fails.
pub fn mrb_call_block(
    vm: &mut VM,
    block: Rc<RObject>,
    recv: Option<Value>,
    args: &[Value],
    return_register: usize,
) -> Result<Value, Error> {
    let block = match &block.value {
        RValue::Proc(p) => p.clone(),
        _ => panic!("Not a block"),
    };
    let recv = match recv {
        Some(r) => r,
        None => block
            .block_self
            .clone()
            .ok_or_else(|| Error::RuntimeError("No block self assigned".to_string()))?,
    };
    vm.push_breadcrumb(
        "block_call",
        None,
        None,
        Some(vm.current_irep.clone()),
        Some(vm.pc.get().saturating_sub(1)),
    );
    let res = if block.is_rb_func {
        call_block(vm, block, recv, args, None, return_register)
    } else if block.is_fnblock {
        let func = vm.pop_fnblock()?;
        let mut buf = arg_buf();
        let mut tmp = Vec::new();
        let res = func(vm, value_args(args, &mut buf, &mut tmp));
        vm.push_fnblock(func)?;
        res
    } else {
        Err(Error::RuntimeError(
            "Cannot call non-block RProc".to_string(),
        ))
    };
    vm.pop_breadcrumb();
    res
}

/// Calls a method on an object by name with the given arguments.
///
/// This is the main function call interface for invoking Ruby methods from Rust code.
///
/// # Arguments
///
/// * `vm` - The virtual machine instance
/// * `top_self` - Optional receiver object. If None, uses the "top self"
/// * `name` - The name of the method to call
/// * `args` - Array of arguments to pass to the method
///
/// # Returns
///
/// Returns the result of the method call or an error if the method is not found or execution fails.
pub fn mrb_funcall(
    vm: &mut VM,
    top_self: Option<Value>,
    name: &str,
    args: &[Value],
) -> Result<Value, Error> {
    let recv: Value = match &top_self {
        Some(v) => v.clone(),
        None => vm.getself()?,
    };
    let binding = recv.singleton_or_this_class(vm);
    let (owner_module, method) = match vm.resolve_method_cached(&binding, name) {
        Some((owner, method)) => (owner, method),
        None => {
            if name == "method_missing" {
                return Err(Error::Internal(
                    "[BUG] method_missing not defined".to_string(),
                ));
            }

            let mut mm_args: Vec<Value> = vec![Value::Symbol(intern_symbol(name))];
            mm_args.extend_from_slice(args);
            return mrb_funcall(vm, top_self, "method_missing", &mm_args);
        }
    };

    let receiver = match recv.rvalue() {
        Some(RValue::Class(c)) => CallerReceiver::Class(c.clone()),
        Some(RValue::Module(m)) => CallerReceiver::Module(m.clone()),
        _ => CallerReceiver::Instance(recv.get_class(vm)),
    };
    // Resolve the frame name to an interned id now; the string is only
    // materialized if this frame ever appears in a backtrace.
    let label_method = method.sym_id.unwrap_or_else(|| intern_symbol(name));
    vm.push_breadcrumb(
        "funcall",
        Some(CallerLabel::Named {
            receiver,
            method_id: label_method,
        }),
        None,
        Some(vm.current_irep.clone()),
        Some(vm.pc.get().saturating_sub(1)),
    );

    let res = if method.is_rb_func {
        let method_id = method.sym_id.unwrap_or_else(|| intern_symbol(name));
        call_block(
            vm,
            method,
            recv.clone(),
            args,
            Some((method_id, owner_module)),
            0, // unused
        )
    } else {
        let prev = vm.current_regs()[0].replace(recv.clone());
        let func = vm.fn_table.get(method.func.unwrap()).unwrap();
        let mut buf = arg_buf();
        let mut tmp = Vec::new();
        let res = func(vm, value_args(args, &mut buf, &mut tmp));
        if let Some(prev) = prev {
            vm.set_reg_value(0, prev);
        } else {
            vm.current_regs()[0].take();
        }

        res
    };
    vm.pop_breadcrumb();

    res
}

pub fn mrb_call_inspect(vm: &mut VM, recv: &Value) -> Result<Value, Error> {
    let binding = recv.get_class(vm);
    let (owner_module, method) = resolve_method(&binding, "inspect")
        .ok_or_else(|| Error::NoMethodError("inspect".to_string()))?;
    if method.is_rb_func {
        let method_id = method.sym_id.unwrap_or_else(|| intern_symbol("inspect"));
        call_block(
            vm,
            method,
            recv.clone(),
            &[],
            Some((method_id, owner_module)),
            0, // unused
        )
    } else {
        let old = vm.current_regs()[0].replace(recv.clone());
        let func = vm.fn_table.get(method.func.unwrap()).unwrap();
        let res = func(vm, &[]);
        if let Some(old) = old {
            vm.set_reg_value(0, old);
        } else {
            vm.current_regs()[0].take();
        }
        res
    }
}

pub fn mrb_call_p(vm: &mut VM, recv: &Value) {
    let inspect = mrb_call_inspect(vm, recv).expect("failed to call inspect");
    let inspect: String = (&inspect).try_into().expect("failed to convert to string");
    eprintln!("{}", inspect);
}

/// Registers a native method into a proc table and bumps the method version.
/// `tag` optionally marks the func for the inline numeric send fast path, and
/// `attr` carries the ivar identity for an attr_accessor fast-path closure.
fn register_cmethod(
    vm: &mut VM,
    procs: &RefCell<RHashMap<u32, RProc>>,
    name: &str,
    tag: Option<FastOp>,
    attr: Option<u32>,
    cmethod: RFn,
) {
    let index = vm.register_fn(cmethod);
    let method = RProc {
        is_rb_func: false,
        is_fnblock: false,
        sym_id: Some(intern_symbol(name)),
        next: None,
        irep: None,
        func: Some(index),
        environ: None,
        block_self: None,
        fast_op: tag,
        attr_key: attr,
    };
    procs.borrow_mut().insert(intern_symbol(name), method);
    vm.bump_method_version();
}

/// Defines a C method (native Rust function) on a Ruby class.
///
/// # Arguments
///
/// * `vm` - The virtual machine instance
/// * `klass` - The class to define the method on
/// * `name` - The name of the method
/// * `cmethod` - The native Rust function to bind as a method
pub fn mrb_define_cmethod(vm: &mut VM, klass: Rc<RClass>, name: &str, cmethod: RFn) {
    register_cmethod(vm, &klass.procs, name, None, None, cmethod);
}

/// Like [`mrb_define_cmethod`], but additionally tags the registered native so
/// `do_op_send` can run it inline on numeric operands. The tag is bound to the
/// func index, so a later redefinition of the same name replaces the method and
/// its tag together.
pub fn mrb_define_cmethod_fast(
    vm: &mut VM,
    klass: Rc<RClass>,
    name: &str,
    op: FastOp,
    cmethod: RFn,
) {
    register_cmethod(vm, &klass.procs, name, Some(op), None, cmethod);
}

/// Like [`mrb_define_cmethod_fast`], for attr_accessor closures: additionally
/// records the `@name` ivar key and its FNV hash so `do_op_send` can execute
/// the getter/setter as a direct IvarMap access with no call at all. `op`
/// must be [`FastOp::AttrGet`] or [`FastOp::AttrSet`].
pub fn mrb_define_cmethod_attr(
    vm: &mut VM,
    klass: Rc<RClass>,
    name: &str,
    op: FastOp,
    key: u32,
    cmethod: RFn,
) {
    register_cmethod(vm, &klass.procs, name, Some(op), Some(key), cmethod);
}

/// Defines a Ruby method (RProc) on a Ruby class.
///
/// # Arguments
///
/// * `vm` - The virtual machine instance
/// * `klass` - The class to define the method on
/// * `name` - The name of the method
/// * `method` - The Ruby proc to bind as a method
pub fn mrb_define_method(vm: &mut VM, klass: Rc<RClass>, name: &str, method: RProc) {
    let mut procs = klass.procs.borrow_mut();
    procs.insert(intern_symbol(name), method);
    vm.bump_method_version();
}

pub fn mrb_define_class_cmethod(vm: &mut VM, klass: Rc<RClass>, name: &str, cmethod: RFn) {
    let klass_singleton = RObject::class_singleton(klass, vm);
    register_cmethod(vm, &klass_singleton.procs, name, None, None, cmethod);
}

/// Defines a singleton C method (native Rust function) on a specific Ruby object.
///
/// Singleton methods are methods defined on individual objects rather than classes.
///
/// # Arguments
///
/// * `vm` - The virtual machine instance
/// * `dest` - The object to define the singleton method on
/// * `name` - The name of the method
/// * `cmethod` - The native Rust function to bind as a singleton method
pub fn mrb_define_singleton_cmethod(vm: &mut VM, dest: Rc<RObject>, name: &str, cmethod: RFn) {
    let klass = dest.initialize_or_get_singleton_class(vm);
    register_cmethod(vm, &klass.procs, name, None, None, cmethod);
}

/// Defines a singleton Ruby method (RProc) on a specific Ruby object.
///
/// Singleton methods are methods defined on individual objects rather than classes.
///
/// # Arguments
///
/// * `vm` - The virtual machine instance
/// * `dest` - The object to define the singleton method on
/// * `name` - The name of the method
/// * `method` - The Ruby proc to bind as a singleton method
pub fn mrb_define_singleton_method(vm: &mut VM, dest: Rc<RObject>, name: &str, method: RProc) {
    let klass = dest.initialize_or_get_singleton_class(vm);
    let mut procs = klass.procs.borrow_mut();
    procs.insert(intern_symbol(name), method);
    vm.bump_method_version();
}

/// Defines a C method (native Rust function) on a Ruby module.
///
/// # Arguments
///
/// * `vm` - The virtual machine instance
/// * `module` - The module to define the method on
/// * `name` - The name of the method
/// * `cmethod` - The native Rust function to bind as a method
pub fn mrb_define_module_cmethod(vm: &mut VM, module: Rc<RModule>, name: &str, cmethod: RFn) {
    register_cmethod(vm, &module.procs, name, None, None, cmethod);
}

/// Defines a Ruby method (RProc) on a Ruby module.
///
/// # Arguments
///
/// * `vm` - The virtual machine instance
/// * `module` - The module to define the method on
/// * `name` - The name of the method
/// * `method` - The Ruby proc to bind as a method
pub fn mrb_define_module_method(vm: &mut VM, module: Rc<RModule>, name: &str, method: RProc) {
    let mut procs = module.procs.borrow_mut();
    procs.insert(intern_symbol(name), method);
    vm.bump_method_version();
}

#[test]
fn test_mrb_inspect() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = VM::empty();
    let old_top_self = RObject::integer(1).to_refcount_assigned();
    vm.set_reg(0, old_top_self.clone());

    let class_a = vm.define_class("A", None, None);
    let class_a = RObject::class(class_a, &mut vm);
    let obj_a = mrb_funcall(&mut vm, Some(Value::from_rc(class_a)), "new", &[])?;
    let res = mrb_call_inspect(&mut vm, &obj_a).unwrap();
    let res_str: String = (&res).try_into().unwrap();
    assert_eq!(
        res_str,
        "#<A:0x".to_string() + &format!("{:016x}", obj_a.object_id()) + ">"
    );

    // assert not to brake registers
    let updated = vm.get_current_regs_cloned(0)?;
    assert_eq!(updated.object_id.get(), old_top_self.object_id.get());

    Ok(())
}
