// Shared conversion helpers and registration macros for the compat layer.

use std::rc::Rc;

use crate::Error;
use crate::yamrb::value::{RClass, RValue, Value};
use crate::yamrb::vm::VM;

pub(crate) use crate::yamrb::helpers::mrb_funcall;

pub(crate) fn nil() -> Value {
    Value::Nil
}

pub(crate) fn ret_bool(b: bool) -> Value {
    Value::Bool(b)
}

pub(crate) fn arg_i64(args: &[Option<Value>], i: usize) -> Result<i64, Error> {
    args.get(i)
        .and_then(|v| v.as_ref())
        .and_then(|v| i64::try_from(v).ok())
        .ok_or_else(|| Error::ArgumentError(format!("expected Integer at arg {i}")))
}

pub(crate) fn arg_f64(args: &[Option<Value>], i: usize) -> Result<f64, Error> {
    args.get(i)
        .and_then(|v| v.as_ref())
        .and_then(|v| f64::try_from(v).ok())
        .ok_or_else(|| Error::ArgumentError(format!("expected number at arg {i}")))
}

pub(crate) fn arg_string(args: &[Option<Value>], i: usize) -> Result<String, Error> {
    match args.get(i).and_then(|v| v.as_ref()) {
        Some(Value::Object(o)) => match &o.value {
            RValue::String(s, _) => Ok(String::from_utf8_lossy(&s.borrow()).to_string()),
            _ => Err(Error::ArgumentError(format!("expected String at arg {i}"))),
        },
        None => Err(Error::ArgumentError(format!("missing arg {i}"))),
        _ => Err(Error::ArgumentError(format!("expected String at arg {i}"))),
    }
}

pub(crate) fn class_rc(vm: &VM, name: &str) -> Result<Rc<RClass>, Error> {
    let obj = vm
        .get_const_by_name(name)
        .ok_or_else(|| Error::NameError(name.to_string()))?;
    match &obj.value {
        RValue::Class(c) => Ok(c.clone()),
        _ => Err(Error::NameError(name.to_string())),
    }
}

macro_rules! native {
    ($kind:ident, $vm:expr, $target:expr, $name:literal, $func:path) => {{
        #[allow(clippy::redundant_clone)]
        let target = $target.clone();
        $kind($vm, target, $name, Box::new($func));
    }};
}

macro_rules! native_fast {
    ($kind:ident, $vm:expr, $target:expr, $name:literal, $op:path, $func:path) => {{
        let target = $target.clone();
        $kind($vm, target, $name, $op, Box::new($func));
    }};
}

pub(crate) use native;
pub(crate) use native_fast;
