// Object/Kernel methods missing from the mrubyedge prelude: reflection over
// instance variables, tap/then, Kernel conversions and print.

use std::rc::Rc;

use crate::compat::util::mrb_funcall;
use crate::Error;
use crate::yamrb::helpers::mrb_define_cmethod;
use crate::yamrb::value::{RClass, RObject, RSym, RValue, Value, intern_symbol, symbol_name};
use crate::yamrb::vm::VM;

use crate::compat::util::{arg_string, class_rc, native};

fn this_obj(vm: &mut VM) -> Result<Value, Error> {
    vm.getself()
}

/// Accepts "x", "@x" or :x / :@x as an ivar name.
fn name_arg(args: &[Option<Value>], i: usize) -> Result<String, Error> {
    let a = args
        .get(i)
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError(format!("missing argument {i}")))?;
    match a {
        Value::Symbol(id) => Ok(ivar_name(&symbol_name(*id))),
        _ => arg_string(args, i).map(|s| ivar_name(&s)),
    }
}

fn ivar_name(raw: &str) -> String {
    match raw.strip_prefix('@') {
        Some(stripped) => format!("@{stripped}"),
        None => format!("@{raw}"),
    }
}

// Object#instance_variable_get(name).
fn cm_ivar_get(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let name = name_arg(args, 0)?;
    let this = this_obj(vm)?;
    Ok(this.get_ivar(name.as_str()))
}

// Object#instance_variable_set(name, value).
fn cm_ivar_set(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let name = name_arg(args, 0)?;
    let value = args
        .get(1)
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("missing value".into()))?;
    let this = this_obj(vm)?;
    // Motorpg patch: immediates are shared, so writing an ivar on one would
    // leak to every instance; MRI forbids it with FrozenError.
    if this.is_immediate() {
        return Err(this.frozen_immediate_error(vm));
    }
    this.set_ivar(name.as_str(), value.clone());
    Ok(value)
}

// Object#instance_variable_defined?(name).
fn cm_ivar_defined(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let name = name_arg(args, 0)?;
    let this = this_obj(vm)?;
    let hit = this
        .to_rc()
        .ivar
        .borrow()
        .contains_key(intern_symbol(name.as_str()));
    Ok(Value::Bool(hit))
}

// Object#instance_variables.
fn cm_instance_variables(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = this_obj(vm)?;
    let mut names: Vec<String> = this.to_rc().ivar.borrow().keys().map(symbol_name).collect();
    names.sort();
    let out = names
        .into_iter()
        .map(|n| Value::from_rc(RObject::symbol(RSym::new(n)).to_refcount_assigned()))
        .collect();
    Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
}

// Object#tap { |x| }: yields self, returns self.
fn cm_tap(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = args
        .last()
        .and_then(|a| a.as_ref())
        .and_then(|a| match a {
            Value::Object(o) if matches!(&o.value, RValue::Proc(_)) => Some(a.clone()),
            _ => None,
        })
        .ok_or_else(|| Error::ArgumentError("no block given".into()))?;
    let this = this_obj(vm)?;
    mrb_funcall(vm, Some(block), "call", std::slice::from_ref(&this))?;
    Ok(this)
}

// Object#then { |x| } (yield_self): returns the block result.
fn cm_then(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = args
        .last()
        .and_then(|a| a.as_ref())
        .and_then(|a| match a {
            Value::Object(o) if matches!(&o.value, RValue::Proc(_)) => Some(a.clone()),
            _ => None,
        })
        .ok_or_else(|| Error::ArgumentError("no block given".into()))?;
    let this = this_obj(vm)?;
    mrb_funcall(vm, Some(block), "call", std::slice::from_ref(&this))
}

// Object#send / __send__: same visibility rules as public_send here.
fn cm_send(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = this_obj(vm)?;
    let rc_args: Vec<Value> = args.iter().map(|a| a.as_ref().unwrap().clone()).collect();
    mrb_funcall(vm, Some(this), "public_send", &rc_args)
}

// Object#eql?(other): equality without numeric cross-type coercion; the
// default == is close enough for this engine.
fn cm_eql(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = this_obj(vm)?;
    let other = args
        .first()
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("eql? requires an argument".into()))?;
    mrb_funcall(vm, Some(this), "==", std::slice::from_ref(&other))
}

// Object#hash: identity-based hash code.
fn cm_object_hash(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = this_obj(vm)?;
    Ok(Value::Integer(this.object_id() as i64))
}

// Object#freeze (documented no-op) and #frozen? (always false in this VM).
fn cm_freeze(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    this_obj(vm)
}

fn cm_frozen(_vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Bool(false))
}

// Kernel#print(*args): to_s of every argument, no trailing newline.
fn cm_print(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let mut line = String::new();
    for a in args {
        if let Some(Value::Object(o)) = a.as_ref()
            && let RValue::String(s, _) = &o.value
        {
            line.push_str(&String::from_utf8_lossy(&s.borrow()));
            continue;
        }
        let text = mrb_funcall(vm, Some(a.as_ref().unwrap().clone()), "to_s", &[])?;
        line.push_str(&String::try_from(&text).unwrap_or_default());
    }
    print!("{line}");
    Ok(Value::Nil)
}

// Kernel#Integer(value): passthrough numbers, strict decimal strings.
fn cm_kernel_integer(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("Integer requires an argument".into()))?;
    match a {
        Value::Integer(n) => Ok(Value::Integer(*n)),
        Value::Float(f) => Ok(Value::Integer(*f as i64)),
        Value::Object(o) => match &o.value {
            RValue::String(b, _) => {
                let s = String::from_utf8_lossy(&b.borrow()).trim().to_string();
                s.parse::<i64>()
                    .map(Value::Integer)
                    .map_err(|_| Error::ArgumentError(format!("invalid value for Integer(): {s}")))
            }
            _ => Err(Error::ArgumentError("invalid value for Integer()".into())),
        },
        _ => Err(Error::ArgumentError("invalid value for Integer()".into())),
    }
}

// Kernel#Float(value).
fn cm_kernel_float(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("Float requires an argument".into()))?;
    match a {
        Value::Integer(n) => Ok(Value::Float(*n as f64)),
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Object(o) => match &o.value {
            RValue::String(b, _) => {
                let s = String::from_utf8_lossy(&b.borrow()).trim().to_string();
                s.parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| Error::ArgumentError(format!("invalid value for Float(): {s}")))
            }
            _ => Err(Error::ArgumentError("invalid value for Float()".into())),
        },
        _ => Err(Error::ArgumentError("invalid value for Float()".into())),
    }
}

// Kernel#String(value): to_s semantics.
fn cm_kernel_string(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = args
        .first()
        .and_then(|v| v.as_ref())
        .cloned()
        .ok_or_else(|| Error::ArgumentError("String requires an argument".into()))?;
    if let Value::Object(o) = &a
        && matches!(o.value, RValue::String(_, _))
    {
        return Ok(a);
    }
    let s = mrb_funcall(vm, Some(a.clone()), "to_s", &[])?;
    Ok(Value::from_rc(Rc::new(RObject::string(
        String::try_from(&s).unwrap_or_default(),
    ))))
}

// Class#< / <= / > / >=: subclass relations over the superclass chain.
fn self_class(vm: &mut VM) -> Result<Rc<RClass>, Error> {
    let this = this_obj(vm)?;
    match this.rvalue() {
        Some(RValue::Class(c)) => Ok(c.clone()),
        _ => Err(Error::RuntimeError("receiver is not a Class".into())),
    }
}

fn arg_class(args: &[Option<Value>]) -> Result<Rc<RClass>, Error> {
    match args.first().and_then(|v| v.as_ref()) {
        Some(Value::Object(o)) => match &o.value {
            RValue::Class(c) => Ok(c.clone()),
            _ => Err(Error::ArgumentError("expected a Class".into())),
        },
        _ => Err(Error::ArgumentError("expected a Class".into())),
    }
}

fn inherits_from(start: &Rc<RClass>, target: &Rc<RClass>) -> bool {
    let mut cur = start.super_class.clone();
    while let Some(c) = cur {
        if Rc::ptr_eq(&c, target) {
            return true;
        }
        cur = c.super_class.clone();
    }
    false
}

fn cmp_class(
    vm: &mut VM,
    args: &[Option<Value>],
    strict: bool,
    forward: bool,
) -> Result<Value, Error> {
    let me = self_class(vm)?;
    let other = arg_class(args)?;
    let same = Rc::ptr_eq(&me, &other);
    let hit = if same {
        !strict
    } else if forward {
        inherits_from(&me, &other)
    } else {
        inherits_from(&other, &me)
    };
    Ok(Value::Bool(hit))
}

fn cm_class_lt(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    cmp_class(vm, args, true, true)
}

fn cm_class_le(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    cmp_class(vm, args, false, true)
}

fn cm_class_gt(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    cmp_class(vm, args, true, false)
}

fn cm_class_ge(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    cmp_class(vm, args, false, false)
}

// Class#<=>: -1 when self inherits other, 1 in the mirror case, 0 on identity.
// Never nil so opcode-level dispatch stays boolean-friendly.
fn cm_class_spaceship(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let me = self_class(vm)?;
    let other = arg_class(args)?;
    let ord = if Rc::ptr_eq(&me, &other) {
        0
    } else if inherits_from(&me, &other) {
        -1
    } else {
        1
    };
    Ok(Value::Integer(ord))
}

// Class#new: exception subclasses are built natively (RException) because
// the stock allocation plus raise pipeline loses the message; everything
// else keeps the allocate-then-initialize flow.
fn cm_class_new(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let klass = self_class(vm)?;
    let is_exception = vm
        .get_const_by_name("Exception")
        .and_then(|o| match &o.value {
            RValue::Class(c) => Some(c.clone()),
            _ => None,
        })
        .map(|exc| inherits_from(&klass, &exc))
        .unwrap_or(false);
    let rc_args: Vec<Value> = args.iter().map(|a| a.as_ref().unwrap().clone()).collect();
    if is_exception {
        return Ok(Value::from_rc(
            crate::compat::exceptions::build_exception_instance(vm, klass, &rc_args)?,
        ));
    }
    let obj = RObject::instance(klass).to_refcount_assigned();
    mrb_funcall(
        vm,
        Some(Value::from_rc(obj.clone())),
        "initialize",
        &rc_args,
    )?;
    Ok(Value::from_rc(obj))
}

// Class#name.
fn cm_class_name(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::from_rc(Rc::new(RObject::string(
        self_class(vm)?.full_name(),
    ))))
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let object = class_rc(vm, "Object")?;
    native!(
        mrb_define_cmethod,
        vm,
        object,
        "instance_variable_get",
        cm_ivar_get
    );
    native!(
        mrb_define_cmethod,
        vm,
        object,
        "instance_variable_set",
        cm_ivar_set
    );
    native!(
        mrb_define_cmethod,
        vm,
        object,
        "instance_variable_defined?",
        cm_ivar_defined
    );
    native!(
        mrb_define_cmethod,
        vm,
        object,
        "instance_variables",
        cm_instance_variables
    );
    native!(mrb_define_cmethod, vm, object, "tap", cm_tap);
    native!(mrb_define_cmethod, vm, object, "then", cm_then);
    native!(mrb_define_cmethod, vm, object, "yield_self", cm_then);
    native!(mrb_define_cmethod, vm, object, "send", cm_send);
    native!(mrb_define_cmethod, vm, object, "__send__", cm_send);
    native!(mrb_define_cmethod, vm, object, "eql?", cm_eql);
    native!(mrb_define_cmethod, vm, object, "hash", cm_object_hash);
    native!(mrb_define_cmethod, vm, object, "freeze", cm_freeze);
    native!(mrb_define_cmethod, vm, object, "frozen?", cm_frozen);
    native!(mrb_define_cmethod, vm, object, "print", cm_print);
    native!(mrb_define_cmethod, vm, object, "Integer", cm_kernel_integer);
    native!(mrb_define_cmethod, vm, object, "Float", cm_kernel_float);
    native!(mrb_define_cmethod, vm, object, "String", cm_kernel_string);

    let class = class_rc(vm, "Class")?;
    native!(mrb_define_cmethod, vm, class, "new", cm_class_new);
    native!(mrb_define_cmethod, vm, class, "<", cm_class_lt);
    native!(mrb_define_cmethod, vm, class, "<=", cm_class_le);
    native!(mrb_define_cmethod, vm, class, ">", cm_class_gt);
    native!(mrb_define_cmethod, vm, class, ">=", cm_class_ge);
    native!(mrb_define_cmethod, vm, class, "name", cm_class_name);
    native!(mrb_define_cmethod, vm, class, "<=>", cm_class_spaceship);
    Ok(())
}
