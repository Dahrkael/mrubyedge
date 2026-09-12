// The Comparable module: derives the comparison family from <=>.

use crate::compat::util::mrb_funcall;
use crate::Error;
use crate::yamrb::prelude::module::mrb_include_module;
use crate::yamrb::value::Value;
use crate::yamrb::vm::VM;

use crate::compat::util::{class_rc, ret_bool};

/// Result of `<=>` as a Rust ordering; None means incomparable.
fn cmp(vm: &mut VM, other: &Value) -> Result<Option<i64>, Error> {
    let this = vm.getself()?;
    let r = mrb_funcall(vm, Some(this), "<=>", std::slice::from_ref(other))?;
    match &r {
        Value::Nil => Ok(None),
        _ => i64::try_from(&r)
            .map(Some)
            .map_err(|_| Error::ArgumentError("bad <=> result".into())),
    }
}

fn cm_comparable_lt(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let o = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("expected an argument".into()))?;
    Ok(ret_bool(matches!(cmp(vm, o)?, Some(v) if v < 0)))
}

fn cm_comparable_le(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let o = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("expected an argument".into()))?;
    Ok(ret_bool(matches!(cmp(vm, o)?, Some(v) if v <= 0)))
}

fn cm_comparable_gt(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let o = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("expected an argument".into()))?;
    Ok(ret_bool(matches!(cmp(vm, o)?, Some(v) if v > 0)))
}

fn cm_comparable_ge(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let o = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("expected an argument".into()))?;
    Ok(ret_bool(matches!(cmp(vm, o)?, Some(v) if v >= 0)))
}

fn cm_comparable_eq(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let o = args
        .first()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| Error::ArgumentError("expected an argument".into()))?;
    Ok(ret_bool(matches!(cmp(vm, o)?, Some(0))))
}

fn arg_pair(args: &[Option<Value>]) -> Result<(Value, Value), Error> {
    match (args.first(), args.get(1)) {
        (Some(lo), Some(hi)) => Ok((lo.as_ref().unwrap().clone(), hi.as_ref().unwrap().clone())),
        _ => Err(Error::ArgumentError("two arguments required".into())),
    }
}

// Comparable#between?(lo, hi).
fn cm_comparable_between(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let (lo, hi) = arg_pair(args)?;
    Ok(match cmp(vm, &lo)? {
        Some(v) if v < 0 => ret_bool(false),
        _ => ret_bool(matches!(cmp(vm, &hi)?, Some(v) if v <= 0)),
    })
}

// Comparable#clamp(lo, hi).
fn cm_comparable_clamp(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let (lo, hi) = arg_pair(args)?;
    if matches!(cmp_pair(vm, &hi, &lo)?, Some(v) if v < 0) {
        return Err(Error::ArgumentError(
            "min argument must be smaller than max".into(),
        ));
    }
    Ok(match cmp(vm, &lo)? {
        Some(v) if v < 0 => lo,
        _ => match cmp(vm, &hi)? {
            Some(v) if v > 0 => hi,
            _ => this,
        },
    })
}

fn cmp_pair(vm: &mut VM, a: &Value, b: &Value) -> Result<Option<i64>, Error> {
    let r = mrb_funcall(vm, Some(a.clone()), "<=>", std::slice::from_ref(b))?;
    match &r {
        Value::Nil => Ok(None),
        _ => i64::try_from(&r)
            .map(Some)
            .map_err(|_| Error::ArgumentError("bad <=> result".into())),
    }
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let comparable = vm.define_module("Comparable", None);
    crate::yamrb::helpers::mrb_define_module_cmethod(
        vm,
        comparable.clone(),
        "<",
        Box::new(cm_comparable_lt),
    );
    crate::yamrb::helpers::mrb_define_module_cmethod(
        vm,
        comparable.clone(),
        "<=",
        Box::new(cm_comparable_le),
    );
    crate::yamrb::helpers::mrb_define_module_cmethod(
        vm,
        comparable.clone(),
        ">",
        Box::new(cm_comparable_gt),
    );
    crate::yamrb::helpers::mrb_define_module_cmethod(
        vm,
        comparable.clone(),
        ">=",
        Box::new(cm_comparable_ge),
    );
    crate::yamrb::helpers::mrb_define_module_cmethod(
        vm,
        comparable.clone(),
        "==",
        Box::new(cm_comparable_eq),
    );
    crate::yamrb::helpers::mrb_define_module_cmethod(
        vm,
        comparable.clone(),
        "between?",
        Box::new(cm_comparable_between),
    );
    crate::yamrb::helpers::mrb_define_module_cmethod(
        vm,
        comparable.clone(),
        "clamp",
        Box::new(cm_comparable_clamp),
    );

    // Core numeric and string types gain between?/clamp through Comparable.
    for name in ["Integer", "Float", "String"] {
        let class = class_rc(vm, name)?;
        mrb_include_module(&class, comparable.clone())?;
    }
    Ok(())
}
