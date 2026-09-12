use std::rc::Rc;

use crate::{Error, yamrb::vm::Breadcrumb};

use super::{
    optable::push_callinfo,
    value::{RClass, RFn, RModule, RObject, RProc, RSym, RValue, resolve_method},
    vm::{CallerLabel, CallerReceiver, VM},
};

fn call_block(
    vm: &mut VM,
    block: RProc,
    recv: Rc<RObject>,
    args: &[Rc<RObject>],
    method_info: Option<(RSym, Rc<RModule>)>,
    return_register: usize,
) -> Result<Rc<RObject>, Error> {
    let (method_id, method_owner) = match method_info {
        Some((id, owner)) => (id, Some(owner)),
        None => (RSym::new("<block>".to_string()), None),
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

    push_callinfo(vm, method_id, args.len(), method_owner, return_register);

    let old_callinfo = vm.current_callinfo.take();

    // Keep the state before the call inside the new window.
    let prev_self = vm.current_regs()[0].replace(recv);

    let mut prev_args = vec![];
    for (i, arg) in args.iter().enumerate() {
        let old = vm.current_regs()[i + 1].replace(arg.clone());
        prev_args.push(old);
    }

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

    if let Some(prev) = prev_self {
        vm.current_regs()[0].replace(prev);
    } else {
        vm.current_regs()[0].take();
    }
    for (i, prev_arg) in prev_args.into_iter().enumerate() {
        if let Some(prev) = prev_arg {
            vm.current_regs()[i + 1].replace(prev);
        } else {
            vm.current_regs()[i + 1].take();
        }
    }

    if let Some(ci) = old_callinfo {
        if let Some(prev) = &ci.prev {
            vm.current_callinfo.replace(prev.clone());
        }
        vm.current_irep = ci.pc_irep.clone();
        vm.pc.set(ci.pc);
        vm.current_regs_offset = saved_offset;
        vm.target_class = ci.target_class.clone();
    }
    vm.upper = prev_upper;

    match &res {
        Ok(res) => Ok(res.clone()),
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
    recv: Option<Rc<RObject>>,
    args: &[Rc<RObject>],
    return_register: usize,
) -> Result<Rc<RObject>, Error> {
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
    let upper = vm.current_breadcrumb.take();
    let new_breadcrumb = Rc::new(Breadcrumb {
        upper,
        event: "block_call",
        caller: None,
        return_reg: None,
        irep: Some(vm.current_irep.clone()),
        pc: Some(vm.pc.get().saturating_sub(1)),
    });
    vm.current_breadcrumb.replace(new_breadcrumb);
    let res = if block.is_rb_func {
        call_block(vm, block, recv, args, None, return_register)
    } else if block.is_fnblock {
        let func = vm.pop_fnblock()?;
        let res = func(vm, args);
        vm.push_fnblock(func)?;
        res
    } else {
        Err(Error::RuntimeError(
            "Cannot call non-block RProc".to_string(),
        ))
    };
    let cur = vm.current_breadcrumb.take().expect("not found breadcrumb");
    if let Some(upper) = &cur.as_ref().upper {
        vm.current_breadcrumb.replace(upper.clone());
    }
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
    top_self: Option<Rc<RObject>>,
    name: &str,
    args: &[Rc<RObject>],
) -> Result<Rc<RObject>, Error> {
    let recv: Rc<RObject> = match &top_self {
        Some(obj) => obj.clone(),
        None => vm.getself()?,
    };
    let binding = recv.singleton_or_this_class(vm);
    let (owner_module, method) = match resolve_method(&binding, name) {
        Some((owner, method)) => (owner, method),
        None => {
            if name == "method_missing" {
                return Err(Error::Internal(
                    "[BUG] method_missing not defined".to_string(),
                ));
            }

            let mut mm_args = vec![RObject::symbol_rc(&RSym::new(name.to_string()))];
            mm_args.extend_from_slice(args);
            return mrb_funcall(vm, top_self, "method_missing", &mm_args);
        }
    };

    let upper = vm.current_breadcrumb.take();
    // lazy frame label; the receiver class is cloned (no
    // allocation) and only formatted when the error stack is captured.
    let receiver = match &recv.value {
        RValue::Class(c) => CallerReceiver::Class(c.clone()),
        RValue::Module(m) => CallerReceiver::Module(m.clone()),
        _ => CallerReceiver::Instance(recv.get_class(vm)),
    };
    let new_breadcrumb = Rc::new(Breadcrumb {
        upper,
        event: "funcall",
        caller: Some(CallerLabel::Named {
            receiver,
            method: name.to_string(),
        }),
        return_reg: None,
        irep: Some(vm.current_irep.clone()),
        pc: Some(vm.pc.get().saturating_sub(1)),
    });
    vm.current_breadcrumb.replace(new_breadcrumb);

    let res = if method.is_rb_func {
        let method_id = method
            .sym_id
            .clone()
            .unwrap_or_else(|| RSym::new(name.to_string()));
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
        let res = func(vm, args);
        if let Some(prev) = prev {
            vm.current_regs()[0].replace(prev);
        } else {
            vm.current_regs()[0].take();
        }

        res
    };
    let cur = vm.current_breadcrumb.take().expect("not found breadcrumb");
    if let Some(upper) = &cur.as_ref().upper {
        vm.current_breadcrumb.replace(upper.clone());
    }

    res
}

pub fn mrb_call_inspect(vm: &mut VM, recv: Rc<RObject>) -> Result<Rc<RObject>, Error> {
    let binding = recv.get_class(vm);
    let (owner_module, method) = resolve_method(&binding, "inspect")
        .ok_or_else(|| Error::NoMethodError("inspect".to_string()))?;
    if method.is_rb_func {
        let method_id = method
            .sym_id
            .clone()
            .unwrap_or_else(|| RSym::new("inspect".to_string()));
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
            vm.current_regs()[0].replace(old);
        } else {
            vm.current_regs()[0].take();
        }
        res
    }
}

pub fn mrb_call_p(vm: &mut VM, recv: Rc<RObject>) {
    let inspect = mrb_call_inspect(vm, recv).expect("failed to call inspect");
    let inspect: String = inspect
        .as_ref()
        .try_into()
        .expect("failed to convert to string");
    eprintln!("{}", inspect);
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
    let index = vm.register_fn(cmethod);
    let method = RProc {
        is_rb_func: false,
        is_fnblock: false,
        sym_id: Some(RSym::new(name.to_string())),
        next: None,
        irep: None,
        func: Some(index),
        environ: None,
        block_self: None,
    };
    let mut procs = klass.procs.borrow_mut();
    procs.insert(name.to_string(), method);
}

/// Defines a Ruby method (RProc) on a Ruby class.
///
/// # Arguments
///
/// * `_vm` - The virtual machine instance (unused)
/// * `klass` - The class to define the method on
/// * `name` - The name of the method
/// * `method` - The Ruby proc to bind as a method
pub fn mrb_define_method(_vm: &mut VM, klass: Rc<RClass>, name: &str, method: RProc) {
    let mut procs = klass.procs.borrow_mut();
    procs.insert(name.to_string(), method);
}

pub fn mrb_define_class_cmethod(vm: &mut VM, klass: Rc<RClass>, name: &str, cmethod: RFn) {
    let index = vm.register_fn(cmethod);
    let method = RProc {
        is_rb_func: false,
        is_fnblock: false,
        sym_id: Some(RSym::new(name.to_string())),
        next: None,
        irep: None,
        func: Some(index),
        environ: None,
        block_self: None,
    };
    let klass_singleton = RObject::class_singleton(klass, vm);
    let mut procs = klass_singleton.procs.borrow_mut();
    procs.insert(name.to_string(), method);
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
    let index = vm.register_fn(cmethod);
    let method = RProc {
        is_rb_func: false,
        is_fnblock: false,
        sym_id: Some(RSym::new(name.to_string())),
        next: None,
        irep: None,
        func: Some(index),
        environ: None,
        block_self: None,
    };
    let klass = dest.initialize_or_get_singleton_class(vm);
    let mut procs = klass.procs.borrow_mut();
    procs.insert(name.to_string(), method);
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
    procs.insert(name.to_string(), method);
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
    let index = vm.register_fn(cmethod);
    let method = RProc {
        is_rb_func: false,
        is_fnblock: false,
        sym_id: Some(RSym::new(name.to_string())),
        next: None,
        irep: None,
        func: Some(index),
        environ: None,
        block_self: None,
    };
    let mut procs = module.procs.borrow_mut();
    procs.insert(name.to_string(), method);
}

/// Defines a Ruby method (RProc) on a Ruby module.
///
/// # Arguments
///
/// * `_vm` - The virtual machine instance (unused)
/// * `module` - The module to define the method on
/// * `name` - The name of the method
/// * `method` - The Ruby proc to bind as a method
pub fn mrb_define_module_method(_vm: &mut VM, module: Rc<RModule>, name: &str, method: RProc) {
    let mut procs = module.procs.borrow_mut();
    procs.insert(name.to_string(), method);
}

#[test]
fn test_mrb_inspect() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = VM::empty();
    let old_top_self = RObject::integer(1).to_refcount_assigned();
    vm.current_regs()[0].replace(old_top_self.clone());

    let class_a = vm.define_class("A", None, None);
    let class_a = RObject::class(class_a, &mut vm);
    let obj_a = mrb_funcall(&mut vm, Some(class_a), "new", &[])?;
    let res = mrb_call_inspect(&mut vm, obj_a.clone()).unwrap();
    let res_str: String = res.as_ref().try_into().unwrap();
    assert_eq!(
        res_str,
        "#<A:0x".to_string() + &format!("{:016x}", obj_a.object_id.get()) + ">"
    );

    // assert not to brake registers
    let updated = vm.get_current_regs_cloned(0)?;
    assert_eq!(updated.object_id.get(), old_top_self.object_id.get());

    Ok(())
}
