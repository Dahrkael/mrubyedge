// Missing exception classes plus Exception#new/message/to_s/inspect.

use std::rc::Rc;

use crate::Error;
use crate::compat::util::mrb_funcall;
use crate::yamrb::helpers::mrb_define_cmethod;
use crate::yamrb::value::{RClass, RException, RObject, RValue, Value};
use crate::yamrb::vm::VM;

use crate::compat::util::class_rc;

fn subclass(vm: &mut VM, name: &str, base: &Rc<RClass>) -> Result<(), Error> {
    vm.define_class(name, Some(base.clone()), None);
    Ok(())
}

// Exception#initialize(msg = nil): stores the text for message/to_s.
fn cm_exception_initialize(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    if let Some(msg) = args.first().and_then(|a| a.as_ref()) {
        this.set_ivar("@__msg", msg.clone());
    }
    Ok(Value::Nil)
}

fn exception_text(vm: &mut VM) -> Result<String, Error> {
    let this = vm.getself()?;
    let stored = this.get_ivar("@__msg");
    if !matches!(stored, Value::Nil) {
        let raw = stored;
        if let Value::Object(o) = &raw
            && let RValue::String(b, _) = &o.value
        {
            return Ok(String::from_utf8_lossy(&b.borrow()).to_string());
        }
        let s = mrb_funcall(vm, Some(raw.clone()), "to_s", &[])?;
        return String::try_from(&s)
            .map_err(|_| Error::RuntimeError("bad exception message".into()));
    }
    match this.rvalue() {
        Some(RValue::Exception(e)) => Ok(e.message.clone()),
        _ => Ok(String::new()),
    }
}

// Exception#message: stored message or the internal one.
fn cm_exception_message(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::from_rc(Rc::new(RObject::string(exception_text(
        vm,
    )?))))
}

// Exception#to_s.
fn cm_exception_to_s(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::from_rc(Rc::new(RObject::string(exception_text(
        vm,
    )?))))
}

// Exception#inspect: "#<ClassName: message>".
fn cm_exception_inspect(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let name = match this.rvalue() {
        Some(RValue::Exception(e)) => e.class.full_name(),
        _ => {
            let klass = this.get_class(vm);
            klass.full_name()
        }
    };
    Ok(Value::from_rc(Rc::new(RObject::string(format!(
        "#<{name}: {}>",
        exception_text(vm)?
    )))))
}

// Exception#new(msg = nil): built natively because the generic Class#new
// path overflows upstream when instantiating exception subclasses.
pub(crate) fn build_exception_instance(
    vm: &mut VM,
    class: Rc<RClass>,
    args: &[Value],
) -> Result<Rc<RObject>, Error> {
    let msg = match args.first() {
        Some(m) => match String::try_from(m) {
            Ok(s) => s,
            Err(_) => {
                let s = mrb_funcall(vm, Some(m.clone()), "to_s", &[])?;
                String::try_from(&s).unwrap_or_default()
            }
        },
        None => class.full_name(),
    };
    let exc = RException {
        class: class.clone(),
        error_type: std::cell::RefCell::new(Error::RuntimeError(msg.clone())),
        message: msg,
        backtrace: Vec::new(),
    };
    Ok(RObject::exception(Rc::new(exc)).to_refcount_assigned())
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    // Classes the prelude lacks; game code rescues them by name.
    let standard = standard_error(vm)?;
    for name in [
        "KeyError",
        "IndexError",
        "StopIteration",
        "FrozenError",
        "IOError",
    ] {
        subclass(vm, name, &standard)?;
    }

    let exc = class_rc(vm, "Exception")?;
    mrb_define_cmethod(
        vm,
        exc.clone(),
        "initialize",
        Box::new(cm_exception_initialize),
    );
    mrb_define_cmethod(vm, exc.clone(), "message", Box::new(cm_exception_message));
    mrb_define_cmethod(vm, exc.clone(), "to_s", Box::new(cm_exception_to_s));
    mrb_define_cmethod(vm, exc, "inspect", Box::new(cm_exception_inspect));
    Ok(())
}

fn standard_error(vm: &mut VM) -> Result<Rc<RClass>, Error> {
    let obj = vm
        .get_const_by_name("StandardError")
        .ok_or_else(|| Error::NameError("StandardError".into()))?;
    match &obj.value {
        RValue::Class(c) => Ok(c.clone()),
        _ => Err(Error::NameError("StandardError".into())),
    }
}
