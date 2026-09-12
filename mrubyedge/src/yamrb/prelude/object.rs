use std::rc::Rc;

use crate::{
    Error,
    yamrb::{
        helpers::{mrb_call_block, mrb_define_cmethod, mrb_define_cmethod_fast, mrb_funcall},
        value::*,
        vm::VM,
    },
};

pub(crate) fn initialize_object(vm: &mut VM) {
    let object_class = vm.object_class.clone();
    let klass = RObject::class(object_class.clone(), vm);
    vm.consts.insert("Object".to_string(), klass);
    vm.builtin_class_table
        .insert("Object", object_class.clone());

    #[cfg(feature = "wasi")]
    {
        mrb_define_cmethod(vm, object_class.clone(), "puts", Box::new(mrb_kernel_puts));
        mrb_define_cmethod(vm, object_class.clone(), "p", Box::new(mrb_kernel_p));
        mrb_define_cmethod(
            vm,
            object_class.clone(),
            "debug",
            Box::new(mrb_kernel_debug),
        );
    }

    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "initialize",
        Box::new(mrb_object_initialize),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "==",
        Box::new(mrb_object_double_eq),
    );
    mrb_define_cmethod_fast(
        vm,
        object_class.clone(),
        "!=",
        FastOp::NumNe,
        Box::new(mrb_object_not_eq),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "===",
        Box::new(mrb_object_triple_eq),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "object_id",
        Box::new(mrb_object_object_id),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "__id__",
        Box::new(mrb_object_object_id),
    );
    mrb_define_cmethod(vm, object_class.clone(), "to_s", Box::new(mrb_object_to_s));
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "inspect",
        Box::new(mrb_object_to_s),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "raise",
        Box::new(mrb_object_raise),
    );
    mrb_define_cmethod(vm, object_class.clone(), "nil?", Box::new(mrb_object_nil_p));
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "block_given?",
        Box::new(mrb_object_block_given),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "lambda",
        Box::new(mrb_object_lambda),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "proc",
        Box::new(mrb_object_lambda),
    );
    mrb_define_cmethod(vm, object_class.clone(), "is_a?", Box::new(mrb_object_is_a));
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "kind_of?",
        Box::new(mrb_object_is_a),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "class",
        Box::new(mrb_object_class),
    );
    mrb_define_cmethod_fast(
        vm,
        object_class.clone(),
        "<=>",
        FastOp::NumSpaceship,
        Box::new(mrb_object_compare),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "method_missing",
        Box::new(mrb_object_method_missing),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "extend",
        Box::new(mrb_object_extend),
    );
    mrb_define_cmethod(vm, object_class.clone(), "loop", Box::new(mrb_object_loop));
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "respond_to?",
        Box::new(mrb_object_respond_to),
    );
    mrb_define_cmethod(
        vm,
        object_class.clone(),
        "public_send",
        Box::new(mrb_object_public_send),
    );

    // define global consts:
    vm.consts.insert(
        "RUBY_VERSION".to_string(),
        Rc::new(RObject::string(crate::yamrb::vm::VERSION.to_string())),
    );
    vm.consts.insert(
        "MRUBY_VERSION".to_string(),
        Rc::new(RObject::string(crate::yamrb::vm::VERSION.to_string())),
    );
    vm.consts.insert(
        "MRUBY_EDGE_VERSION".to_string(),
        Rc::new(RObject::string(crate::yamrb::vm::VERSION.to_string())),
    );
    vm.consts.insert(
        "RUBY_ENGINE".to_string(),
        Rc::new(RObject::string(crate::yamrb::vm::ENGINE.to_string())),
    );
    mrb_define_cmethod(vm, object_class.clone(), "wasm?", Box::new(mrb_is_wasm));
}

pub fn mrb_self(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    vm.getself()
}

#[cfg(feature = "wasi")]
pub fn mrb_kernel_puts(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let msg = args[0].as_ref().unwrap().clone();
    match &msg {
        Value::Integer(i) => {
            println!("{}", i);
        }
        Value::Object(o) => match &o.value {
            RValue::String(s, _) => {
                println!("{}", String::from_utf8_lossy(&s.borrow()));
            }
            _ => {
                let inspect = mrb_funcall(vm, Some(msg.clone()), "to_s", &[])?;
                let inspect: String = (&inspect).try_into()?;
                println!("{}", inspect);
            }
        },
        _ => {
            let inspect = mrb_funcall(vm, Some(msg.clone()), "to_s", &[])?;
            let inspect: String = (&inspect).try_into()?;
            println!("{}", inspect);
        }
    }
    Ok(Value::Nil)
}

#[cfg(feature = "wasi")]
pub fn mrb_kernel_p(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let msg = args[0].as_ref().unwrap().clone();
    let inspect = mrb_funcall(vm, Some(msg), "inspect", &[])?;
    let inspect: String = (&inspect).try_into()?;
    println!("{}", inspect);
    Ok(Value::Nil)
}

#[cfg(feature = "wasi")]
pub fn mrb_kernel_debug(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    for (i, obj) in args.iter().enumerate() {
        dbg!(i, obj.clone());
    }
    Ok(Value::Nil)
}

// honor custom #== overrides. Primitive pairs keep the
// structural fast path; otherwise dispatch unless only the Object default
// (which would recurse back into this function) resolves.
pub fn mrb_object_is_equal(vm: &mut VM, lhs: Value, rhs: Value) -> Value {
    let structural = || Value::Bool(lhs.to_rc().as_eq_value() == rhs.to_rc().as_eq_value());
    let primitive = |v: &Value| match v {
        Value::Integer(_) | Value::Float(_) | Value::Bool(_) | Value::Nil | Value::Symbol(_) => {
            true
        }
        Value::Object(o) => matches!(o.value, RValue::String(..)),
    };
    if primitive(&lhs) && primitive(&rhs) {
        return structural();
    }
    if let Some((owner, _method)) = resolve_method(&lhs.get_class(vm), "==")
        && owner.sym_id.name != "Object"
        && let Ok(r) = mrb_funcall(vm, Some(lhs.clone()), "==", std::slice::from_ref(&rhs))
    {
        return r;
    }
    structural()
}

pub fn mrb_object_is_not_equal(_vm: &mut VM, lhs: Value, rhs: Value) -> Value {
    Value::Bool(lhs.to_rc().as_eq_value() != rhs.to_rc().as_eq_value())
}

pub fn mrb_object_double_eq(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap().clone();
    Ok(mrb_object_is_equal(vm, lhs, rhs))
}

pub fn mrb_object_not_eq(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap().clone();
    Ok(mrb_object_is_not_equal(vm, lhs, rhs))
}

pub fn mrb_object_triple_eq(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap().clone();

    match (&lhs, &rhs) {
        (Value::Integer(i1), Value::Integer(i2)) => Ok(Value::Bool(i1 == i2)),
        (Value::Float(f1), Value::Float(f2)) => Ok(Value::Bool(f1 == f2)),
        (Value::Symbol(sym1), Value::Symbol(sym2)) => Ok(Value::Bool(sym1 == sym2)),
        (Value::Object(o1), _) => match &o1.value {
            RValue::String(s1, _) => match &rhs {
                Value::Object(o2) => match &o2.value {
                    RValue::String(s2, _) => Ok(Value::Bool(s1 == s2)),
                    _ => Ok(Value::Bool(false)),
                },
                _ => Ok(Value::Bool(false)),
            },
            RValue::Class(c1) => {
                let c2 = lhs.get_class(vm);
                Ok(Value::Bool(c1.sym_id == c2.sym_id))
            }
            RValue::Range(_s, _e, _v) => {
                let arg = vec![rhs.clone()];
                mrb_funcall(vm, Some(lhs.clone()), "include?", &arg)
            }
            // TODO: Implement object id for generic instance
            _ => Ok(Value::Bool(false)),
        },
        _ => Ok(Value::Bool(false)),
    }
}

// Object#<=>: Comparison operator (returns -1, 0, or 1)
pub fn mrb_object_compare(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap().clone();

    // Use as_eq_value for comparison
    let lhs_val = lhs.as_eq_value();
    let rhs_val = rhs.as_eq_value();

    use ValueEquality::*;
    let result = match (&lhs_val, &rhs_val) {
        (Integer(a), Integer(b)) => {
            if a < b {
                -1
            } else if a > b {
                1
            } else {
                0
            }
        }
        (Float(a), Float(b)) => {
            if a < b {
                -1
            } else if a > b {
                1
            } else {
                0
            }
        }
        (String(a), String(b)) => {
            if a < b {
                -1
            } else if a > b {
                1
            } else {
                0
            }
        }
        (Integer(a), Float(b)) => {
            let a_float = *a as f64;
            if a_float < *b {
                -1
            } else if a_float > *b {
                1
            } else {
                0
            }
        }
        (Float(a), Integer(b)) => {
            let b_float = *b as f64;
            if a < &b_float {
                -1
            } else if a > &b_float {
                1
            } else {
                0
            }
        }
        (Symbol(a), Symbol(b)) => {
            if a < b {
                -1
            } else if a > b {
                1
            } else {
                0
            }
        }
        _ => {
            return Err(Error::ArgumentError(
                "comparison of incompatible types".to_string(),
            ));
        }
    };

    Ok(Value::Integer(result))
}

pub fn mrb_object_object_id(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    // Abstract method; do nothing
    let x = vm.getself()?.object_id();
    // ref: https://stackoverflow.com/questions/74491204/how-do-i-represent-an-i64-in-the-u64-domain
    let to_i64 = ((x as i64) ^ (1 << 63)) & (1 << 63) | (x & (u64::MAX >> 1)) as i64;
    Ok(Value::Integer(to_i64))
}

pub fn mrb_object_to_s(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let obj = vm.getself()?;
    if obj.is_main() {
        return Ok(Value::from_rc(
            RObject::string("main".to_string()).to_refcount_assigned(),
        ));
    }
    let class = obj.get_class(vm);
    let obj_rc = obj.to_rc();
    let addr = format!("{:018p}", Rc::as_ptr(&obj_rc));
    Ok(Value::from_rc(
        RObject::string(format!("#<{}:{}>", class.full_name(), addr)).to_refcount_assigned(),
    ))
}

pub fn mrb_object_raise(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let klass = args.first().and_then(|a| a.as_ref()).and_then(|v| match v {
        Value::Object(o) => match &o.value {
            RValue::Class(k) => Some(k.clone()),
            _ => None,
        },
        _ => None,
    });
    if let Some(klass) = klass {
        // raise ImageNotFoundError, "title not found" (or raise SomeClass)
        let class_name = klass.full_name();
        let msg = args
            .get(1)
            .map(|a| String::try_from(a.as_ref().unwrap()).unwrap_or_default())
            .unwrap_or_else(|| class_name.clone());
        return Err(Error::TaggedError(class_name, msg));
    }
    let msg = args
        .first()
        .map(|a| String::try_from(a.as_ref().unwrap()).unwrap_or_default())
        .unwrap_or_default();
    Err(Error::RuntimeError(msg))
}

fn mrb_object_nil_p(_vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Bool(false))
}

fn mrb_object_block_given(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    // CALLINFO の has_block フラグをチェック
    let has_block = if let Some(ci) = vm.callinfo_stack.last() {
        ci.has_block.get()
    } else {
        false
    };

    Ok(Value::Bool(has_block))
}

pub fn mrb_object_initialize(_vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    // Abstract method; do nothing
    Ok(Value::Nil)
}

pub fn mrb_object_lambda(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let proc = args[args.len() - 1].as_ref().unwrap().clone();
    if matches!(&proc, Value::Object(o) if matches!(o.value, RValue::Proc(_))) {
        Ok(proc)
    } else {
        Err(Error::RuntimeError(
            "Object#lambda expects a Proc as the last argument".to_string(),
        ))
    }
}

fn mrb_object_is_a(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let obj = vm.getself()?;
    let class_arg = args[0].as_ref().unwrap().clone();
    let is_a = match &class_arg {
        Value::Object(o) => match &o.value {
            RValue::Class(c) => mrb_is_a(vm, &obj, c.clone()),
            RValue::Module(m) => mrb_is_a(vm, &obj, m.clone()),
            _ => {
                return Err(Error::ArgumentError(
                    "Object#is_a? expects a Class or Module".to_string(),
                ));
            }
        },
        _ => {
            return Err(Error::ArgumentError(
                "Object#is_a? expects a Class or Module".to_string(),
            ));
        }
    };
    Ok(Value::Bool(is_a))
}

fn mrb_object_class(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let obj = vm.getself()?;
    let class = obj.get_class(vm);
    Ok(Value::from_rc(RObject::class_or_module(
        class.as_module(),
        vm,
    )))
}

fn mrb_object_loop(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = args[0].as_ref().unwrap().clone();
    if !matches!(&block, Value::Object(o) if matches!(o.value, RValue::Proc(_))) {
        return Err(Error::ArgumentError(
            "Object#loop expects a block".to_string(),
        ));
    }

    let this = vm.getself()?;
    loop {
        match mrb_call_block(vm, block.to_rc(), Some(this.clone()), &[], 0) {
            Ok(_) => {}
            // break inside the block stops loop and its
            // value becomes the method result (Ruby semantics). Consume the
            // pending exception (see integer.rs note).
            Err(Error::Break(v)) => {
                vm.exception.take();
                return Ok(v);
            }
            Err(e) => return Err(e),
        }
    }
}

fn mrb_object_respond_to(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let method_name: String = args[0].as_ref().unwrap().try_into()?;
    let obj = vm.getself()?;
    let klass = obj.singleton_or_this_class(vm);
    let has_method = resolve_method(&klass, &method_name).is_some();
    Ok(Value::Bool(has_method))
}

fn mrb_object_public_send(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    if args.is_empty() {
        return Err(Error::ArgumentError(
            "wrong number of arguments (given 0, expected 1+)".to_string(),
        ));
    }

    let method_name: String = args[0].as_ref().unwrap().try_into()?;
    let obj = vm.getself()?;
    let method_args = args[1..]
        .iter()
        .map(|a| a.as_ref().unwrap().clone())
        .collect::<Vec<_>>();

    // For now, public_send behaves the same as send since we don't have visibility modifiers
    mrb_funcall(vm, Some(obj), &method_name, &method_args)
}

fn mrb_object_method_missing(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let method_name_obj = &args
        .first()
        .ok_or_else(|| Error::Internal("[BUG] method_missing without any args".to_string()))?;
    let method_name: String = method_name_obj
        .as_ref()
        .unwrap()
        .to_rc()
        .as_ref()
        .try_into()?;
    Err(Error::NoMethodError(format!(
        "undefined method `{}` for {}",
        method_name,
        vm.getself()?.get_class(vm).full_name()
    )))
}

pub fn mrb_is_a(vm: &mut VM, obj: &Value, class: impl AsModule) -> bool {
    let obj_class = obj.get_class(vm);
    let target_module = class.as_module();
    for module in build_lookup_chain(&obj_class).iter() {
        if Rc::ptr_eq(module, &target_module) {
            return true;
        }
    }
    false
}

fn mrb_is_wasm(_vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let is_wasm = cfg!(target_arch = "wasm32");
    Ok(Value::Bool(is_wasm))
}

#[test]
fn test_mrb_object_is_equal() {
    let mut vm = VM::empty();

    let lhs = RObject::integer(1).to_refcount_assigned();
    let rhs = RObject::integer(1).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = RObject::integer(1).to_refcount_assigned();
    let rhs = RObject::integer(3).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let lhs = RObject::string("mruby/edge is Ruby".into()).to_refcount_assigned();
    let rhs = RObject::string("mruby/edge is Ruby".into()).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = RObject::string("mruby/edge is Ruby".into()).to_refcount_assigned();
    let rhs = RObject::string("mruby/edge is not Ruby".into()).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let lhs = RObject::symbol("some".into()).to_refcount_assigned();
    let rhs = RObject::symbol("some".into()).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = RObject::symbol("some".into()).to_refcount_assigned();
    let rhs = RObject::symbol("other".into()).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let lhs = RObject::boolean(true).to_refcount_assigned();
    let rhs = RObject::boolean(true).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = RObject::boolean(false).to_refcount_assigned();
    let rhs = RObject::boolean(false).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = RObject::boolean(true).to_refcount_assigned();
    let rhs = RObject::boolean(false).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let lhs = RObject::float(0.1).to_refcount_assigned();
    let rhs = RObject::float(0.1).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = RObject::float(0.2).to_refcount_assigned();
    let rhs = RObject::float(0.1).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let lhs = RObject::nil().to_refcount_assigned();
    let rhs = RObject::nil().to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = RObject::integer(100).to_refcount_assigned();
    let rhs = RObject::nil().to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let lhs = RObject::integer(100).to_refcount_assigned();
    let rhs = RObject::nil().to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);
}

#[test]
fn test_mrb_object_is_equal_range() {
    let mut vm = VM::empty();

    let s = RObject::integer(1).to_refcount_assigned();
    let e = RObject::integer(10).to_refcount_assigned();
    let lhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e.clone()), true)
        .to_refcount_assigned();
    let rhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e.clone()), true)
        .to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let s = RObject::integer(1).to_refcount_assigned();
    let e = RObject::integer(10).to_refcount_assigned();
    let lhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e.clone()), true)
        .to_refcount_assigned();
    let rhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e.clone()), false)
        .to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let s = RObject::integer(1).to_refcount_assigned();
    let e = RObject::integer(10).to_refcount_assigned();
    let e2 = RObject::integer(11).to_refcount_assigned();
    let lhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e.clone()), true)
        .to_refcount_assigned();
    let rhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e2.clone()), true)
        .to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let s = RObject::string("a".into()).to_refcount_assigned();
    let e = RObject::string("z".into()).to_refcount_assigned();
    let lhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e.clone()), true)
        .to_refcount_assigned();
    let rhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e.clone()), true)
        .to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let s = RObject::string("a".into()).to_refcount_assigned();
    let e = RObject::string("z".into()).to_refcount_assigned();
    let e2 = RObject::string("A".into()).to_refcount_assigned();
    let lhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e.clone()), true)
        .to_refcount_assigned();
    let rhs = RObject::range(Value::from_rc(s.clone()), Value::from_rc(e2.clone()), true)
        .to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);
}

#[test]
fn test_mrb_object_is_equal_array() {
    let mut vm = VM::empty();

    let lhs = RObject::array(vec![]).to_refcount_assigned();
    let rhs = RObject::array(vec![]).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)];
    let rhs = vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)];
    let lhs = RObject::array(lhs).to_refcount_assigned();
    let rhs = RObject::array(rhs).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)];
    let rhs = vec![Value::Integer(1), Value::Integer(2), Value::Integer(4)];
    let lhs = RObject::array(lhs).to_refcount_assigned();
    let rhs = RObject::array(rhs).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);
}

#[test]
fn test_mrb_object_is_equal_hash() {
    use crate::yamrb::prelude::hash::*;

    let mut vm = VM::empty();

    let lhs = Value::from_rc(RObject::hash(RHashMap::default()).to_refcount_assigned());
    let rhs = Value::from_rc(RObject::hash(RHashMap::default()).to_refcount_assigned());
    let ret: bool = mrb_object_is_equal(&mut vm, lhs.clone(), rhs.clone())
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = Value::from_rc(RObject::hash(RHashMap::default()).to_refcount_assigned());
    mrb_hash_set_index(
        &lhs,
        Value::from_rc(RObject::symbol("key1".into()).to_refcount_assigned()),
        Value::Integer(1),
    )
    .expect("set index failed");
    mrb_hash_set_index(
        &lhs,
        Value::from_rc(RObject::symbol("key2".into()).to_refcount_assigned()),
        Value::Integer(2),
    )
    .expect("set index failed");

    let rhs = Value::from_rc(RObject::hash(RHashMap::default()).to_refcount_assigned());
    mrb_hash_set_index(
        &rhs,
        Value::from_rc(RObject::symbol("key2".into()).to_refcount_assigned()),
        Value::Integer(2),
    )
    .expect("set index failed");
    mrb_hash_set_index(
        &rhs,
        Value::from_rc(RObject::symbol("key1".into()).to_refcount_assigned()),
        Value::Integer(1),
    )
    .expect("set index failed");

    let ret: bool = mrb_object_is_equal(&mut vm, lhs.clone(), rhs.clone())
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = Value::from_rc(RObject::hash(RHashMap::default()).to_refcount_assigned());
    mrb_hash_set_index(
        &lhs,
        Value::from_rc(RObject::symbol("key1".into()).to_refcount_assigned()),
        Value::Integer(1),
    )
    .expect("set index failed");
    mrb_hash_set_index(
        &lhs,
        Value::from_rc(RObject::symbol("key2".into()).to_refcount_assigned()),
        Value::Integer(2),
    )
    .expect("set index failed");

    let rhs = Value::from_rc(RObject::hash(RHashMap::default()).to_refcount_assigned());
    mrb_hash_set_index(
        &rhs,
        Value::from_rc(RObject::symbol("key2".into()).to_refcount_assigned()),
        Value::Integer(2),
    )
    .expect("set index failed");
    mrb_hash_set_index(
        &rhs,
        Value::from_rc(RObject::symbol("key1".into()).to_refcount_assigned()),
        Value::Integer(3),
    )
    .expect("set index failed");

    let ret: bool = mrb_object_is_equal(&mut vm, lhs.clone(), rhs.clone())
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);

    let lhs = Value::from_rc(RObject::hash(RHashMap::default()).to_refcount_assigned());
    mrb_hash_set_index(
        &lhs,
        Value::from_rc(RObject::symbol("key1".into()).to_refcount_assigned()),
        Value::Integer(1),
    )
    .expect("set index failed");
    mrb_hash_set_index(
        &lhs,
        Value::from_rc(RObject::symbol("key2".into()).to_refcount_assigned()),
        Value::Integer(2),
    )
    .expect("set index failed");

    let rhs = Value::from_rc(RObject::hash(RHashMap::default()).to_refcount_assigned());
    mrb_hash_set_index(
        &rhs,
        Value::from_rc(RObject::symbol("key2".into()).to_refcount_assigned()),
        Value::Integer(2),
    )
    .expect("set index failed");
    mrb_hash_set_index(
        &rhs,
        Value::from_rc(RObject::symbol("key1-b".into()).to_refcount_assigned()),
        Value::Integer(1),
    )
    .expect("set index failed");

    let ret: bool = mrb_object_is_equal(&mut vm, lhs.clone(), rhs.clone())
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);
}

#[test]
fn test_mrb_object_is_equal_klass() {
    let mut vm = VM::empty();

    let lhs: Rc<RClass> = vm.get_class_by_name("String");
    let rhs: Rc<RClass> = RObject::string("String".into()).get_class(&vm);
    let lhs = RObject::class(lhs.clone(), &mut vm);
    let rhs = RObject::class(rhs.clone(), &mut vm);
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs: Rc<RClass> = RObject::integer(5471).get_class(&vm);
    let rhs: Rc<RClass> = RObject::string("String".into()).get_class(&vm);
    let lhs = RObject::class(lhs.clone(), &mut vm);
    let rhs = RObject::class(rhs.clone(), &mut vm);
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);
}

#[test]
fn test_mrb_object_is_equal_instance() {
    let mut vm = VM::empty();

    let lhs = RObject::instance(vm.object_class.clone()).to_refcount_assigned();
    let rhs = lhs.clone();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(ret);

    let lhs = RObject::instance(vm.object_class.clone()).to_refcount_assigned();
    let rhs = RObject::instance(vm.object_class.clone()).to_refcount_assigned();
    let ret: bool = mrb_object_is_equal(&mut vm, Value::from_rc(lhs), Value::from_rc(rhs))
        .to_rc()
        .as_ref()
        .try_into()
        .expect("must return bool");
    assert!(!ret);
}

fn mrb_object_extend(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;

    if args.is_empty() {
        return Err(Error::ArgumentError(
            "wrong number of arguments (given 0, expected 1+)".to_string(),
        ));
    }

    // Initialize or get singleton class
    let singleton_class = this.initialize_or_get_singleton_class(vm);

    // Extend with each module argument
    for arg in args.iter().rev().map(|a| a.as_ref().unwrap().clone()) {
        let module = match &arg {
            Value::Object(o) => match &o.value {
                RValue::Module(m) => m.clone(),
                RValue::Class(c) => c.module.clone(),
                _ => {
                    return Err(Error::ArgumentError(
                        "wrong argument type (expected Module)".to_string(),
                    ));
                }
            },
            Value::Nil => continue,
            _ => {
                return Err(Error::ArgumentError(
                    "wrong argument type (expected Module)".to_string(),
                ));
            }
        };

        // Add module to extended_modules
        singleton_class
            .extended_modules
            .borrow_mut()
            .insert(0, module);
    }

    Ok(this)
}
