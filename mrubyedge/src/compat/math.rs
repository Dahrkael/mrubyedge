// Math module: trigonometry and friends, missing from the mrubyedge prelude.

use std::rc::Rc;

use crate::Error;
use crate::yamrb::helpers::mrb_define_singleton_cmethod;
use crate::yamrb::value::{RObject, Value};
use crate::yamrb::vm::VM;

use crate::compat::util::arg_f64;

type CMethod = fn(&mut VM, &[Option<Value>]) -> Result<Value, Error>;

fn unary(args: &[Option<Value>], f: impl Fn(f64) -> f64) -> Result<Value, Error> {
    Ok(Value::Float(f(arg_f64(args, 0)?)))
}

fn binary(args: &[Option<Value>], f: impl Fn(f64, f64) -> f64) -> Result<Value, Error> {
    let a = arg_f64(args, 0)?;
    let b = arg_f64(args, 1)?;
    Ok(Value::Float(f(a, b)))
}

fn domain(v: f64, lo: f64, hi: f64, name: &str) -> Result<f64, Error> {
    if (lo..=hi).contains(&v) {
        Ok(v)
    } else {
        Err(Error::RangeError(format!("{name} out of domain")))
    }
}

fn cm_math_sin(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::sin)
}

fn cm_math_cos(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::cos)
}

fn cm_math_tan(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::tan)
}

fn cm_math_asin(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Float(
        domain(arg_f64(args, 0)?, -1.0, 1.0, "asin")?.asin(),
    ))
}

fn cm_math_acos(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Float(
        domain(arg_f64(args, 0)?, -1.0, 1.0, "acos")?.acos(),
    ))
}

fn cm_math_atan(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::atan)
}

fn cm_math_atan2(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    binary(args, f64::atan2)
}

fn cm_math_sinh(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::sinh)
}

fn cm_math_cosh(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::cosh)
}

fn cm_math_tanh(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::tanh)
}

fn cm_math_sqrt(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let v = arg_f64(args, 0)?;
    if v < 0.0 {
        return Err(Error::RangeError("sqrt of negative number".into()));
    }
    Ok(Value::Float(v.sqrt()))
}

fn cm_math_cbrt(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::cbrt)
}

fn cm_math_exp(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::exp)
}

fn cm_math_log(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let v = arg_f64(args, 0)?;
    // Ruby returns -Infinity for log(0); only negatives raise.
    if v < 0.0 {
        return Err(Error::RangeError("log of negative number".into()));
    }
    let value = match args.len() {
        1 => v.ln(),
        _ => {
            let base = arg_f64(args, 1)?;
            if base <= 0.0 || base == 1.0 {
                return Err(Error::RangeError("bad log base".into()));
            }
            v.log(base)
        }
    };
    Ok(Value::Float(value))
}

fn cm_math_log2(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let v = arg_f64(args, 0)?;
    if v < 0.0 {
        return Err(Error::RangeError("log2 of negative number".into()));
    }
    Ok(Value::Float(v.log2()))
}

fn cm_math_log10(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let v = arg_f64(args, 0)?;
    if v < 0.0 {
        return Err(Error::RangeError("log10 of negative number".into()));
    }
    Ok(Value::Float(v.log10()))
}

fn cm_math_hypot(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    binary(args, f64::hypot)
}

fn cm_math_pow(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    binary(args, f64::powf)
}

fn cm_math_abs(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    unary(args, f64::abs)
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let math = vm.define_module("Math", None);
    let math_obj = vm
        .get_const_by_name("Math")
        .expect("Math just defined must exist in consts");

    let fns: &[(&str, CMethod)] = &[
        ("sin", cm_math_sin),
        ("cos", cm_math_cos),
        ("tan", cm_math_tan),
        ("asin", cm_math_asin),
        ("acos", cm_math_acos),
        ("atan", cm_math_atan),
        ("atan2", cm_math_atan2),
        ("sinh", cm_math_sinh),
        ("cosh", cm_math_cosh),
        ("tanh", cm_math_tanh),
        ("sqrt", cm_math_sqrt),
        ("cbrt", cm_math_cbrt),
        ("exp", cm_math_exp),
        ("log", cm_math_log),
        ("log2", cm_math_log2),
        ("log10", cm_math_log10),
        ("hypot", cm_math_hypot),
        ("pow", cm_math_pow),
        ("abs", cm_math_abs),
    ];
    for (name, func) in fns {
        mrb_define_singleton_cmethod(vm, math_obj.clone(), name, Box::new(*func));
    }

    let consts = [("PI", std::f64::consts::PI), ("E", std::f64::consts::E)];
    for (name, value) in consts {
        math.consts
            .borrow_mut()
            .insert(name.to_string(), Rc::new(RObject::float(value)));
    }
    Ok(())
}
