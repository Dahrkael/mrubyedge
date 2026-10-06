use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::Signed;

use crate::Error;
use crate::yamrb::helpers::{mrb_define_cmethod, mrb_define_cmethod_fast};

use crate::yamrb::value::{FastOp, Value, bigint_to_f64};
use crate::yamrb::{helpers::mrb_call_block, value::RObject, vm::VM};

pub(crate) fn initialize_integer(vm: &mut VM) {
    let integer_class = vm.define_standard_class("Integer");

    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "[]",
        Box::new(mrb_integer_bitref),
    );
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "-@",
        Box::new(mrb_integer_negative),
    );
    mrb_define_cmethod(vm, integer_class.clone(), "+", Box::new(mrb_integer_add));
    mrb_define_cmethod(vm, integer_class.clone(), "-", Box::new(mrb_integer_sub));
    mrb_define_cmethod_fast(
        vm,
        integer_class.clone(),
        "**",
        FastOp::IntPow,
        Box::new(mrb_integer_power),
    );
    mrb_define_cmethod_fast(
        vm,
        integer_class.clone(),
        "%",
        FastOp::IntModTrunc,
        Box::new(mrb_integer_mod),
    );
    mrb_define_cmethod(vm, integer_class.clone(), "&", Box::new(mrb_integer_and));
    mrb_define_cmethod(vm, integer_class.clone(), "|", Box::new(mrb_integer_or));
    mrb_define_cmethod(vm, integer_class.clone(), "^", Box::new(mrb_integer_xor));
    mrb_define_cmethod(vm, integer_class.clone(), "~", Box::new(mrb_integer_not));
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "<<",
        Box::new(mrb_integer_lshift),
    );
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        ">>",
        Box::new(mrb_integer_rshift),
    );
    mrb_define_cmethod(vm, integer_class.clone(), "abs", Box::new(mrb_integer_abs));
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "to_i",
        Box::new(mrb_integer_to_i),
    );
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "to_f",
        Box::new(mrb_integer_to_f),
    );
    mrb_define_cmethod(vm, integer_class.clone(), "chr", Box::new(mrb_integer_chr));
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "times",
        Box::new(mrb_integer_times),
    );
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "inspect",
        Box::new(mrb_integer_inspect),
    );
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "to_s",
        Box::new(mrb_integer_inspect),
    );
    mrb_define_cmethod(
        vm,
        integer_class.clone(),
        "clamp",
        Box::new(mrb_integer_clamp),
    );
}

fn mrb_integer_inspect(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match &this {
        Value::Integer(n) => Ok(Value::from_rc(Rc::new(RObject::string(n.to_string())))),
        Value::BigInt(b) => Ok(Value::from_rc(Rc::new(RObject::string(b.to_string())))),
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_times(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this: i64 = vm.getself()?.try_into()?;
    for i in 0..this {
        let block = args[0].as_ref().unwrap().clone();
        let args = vec![Value::Integer(i)];
        match mrb_call_block(vm, block.to_rc(), None, &args, 0) {
            Ok(_) => {}
            // break inside the block stops the iterator and
            // its value becomes the method result (Ruby semantics). The
            // pending exception is consumed here — leaving it set would
            // re-fire as a phantom error in the enclosing loop.
            Err(Error::Break(v)) => {
                vm.exception.take();
                return Ok(v);
            }
            Err(e) => return Err(e),
        }
    }
    vm.getself()
}

fn mrb_integer_mod(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs: i64 = vm.getself()?.try_into()?;
    let rhs: i64 = args[0].as_ref().unwrap().try_into()?;

    Ok(Value::Integer(lhs % rhs))
}

fn mrb_integer_bitref(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this: i64 = vm.getself()?.try_into()?;
    let index: i64 = args[0].as_ref().unwrap().try_into()?;

    if index < 0 {
        return Ok(Value::Integer(0));
    }

    let bit = (this >> index) & 1;
    Ok(Value::Integer(bit))
}

fn mrb_integer_negative(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match &this {
        Value::Integer(n) => Ok(Value::Integer(-n)),
        Value::BigInt(b) => Ok(Value::BigInt(Rc::new(-b.as_ref()))),
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_add(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap();

    match (&lhs, rhs) {
        (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l + r)),
        (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 + r)),
        (Value::Integer(l), Value::BigInt(r)) => {
            Ok(Value::BigInt(Rc::new(BigInt::from(*l) + r.as_ref())))
        }
        (Value::BigInt(l), Value::BigInt(r)) => Ok(Value::BigInt(Rc::new(l.as_ref() + r.as_ref()))),
        (Value::BigInt(l), Value::Integer(r)) => {
            Ok(Value::BigInt(Rc::new(l.as_ref() + BigInt::from(*r))))
        }
        (Value::BigInt(l), Value::Float(r)) => Ok(Value::Float(bigint_to_f64(l) + r)),
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_sub(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap();

    match (&lhs, rhs) {
        (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l - r)),
        (Value::Integer(l), Value::Float(r)) => Ok(Value::Float(*l as f64 - r)),
        (Value::Integer(l), Value::BigInt(r)) => {
            Ok(Value::BigInt(Rc::new(BigInt::from(*l) - r.as_ref())))
        }
        (Value::BigInt(l), Value::BigInt(r)) => Ok(Value::BigInt(Rc::new(l.as_ref() - r.as_ref()))),
        (Value::BigInt(l), Value::Integer(r)) => {
            Ok(Value::BigInt(Rc::new(l.as_ref() - BigInt::from(*r))))
        }
        (Value::BigInt(l), Value::Float(r)) => Ok(Value::Float(bigint_to_f64(l) - r)),
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_power(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let base: i64 = vm.getself()?.try_into()?;
    let exponent_obj = args[0].as_ref().unwrap();

    match exponent_obj {
        Value::Integer(exp) => {
            if *exp >= 0 {
                // Positive integer exponent
                let result = base.pow(*exp as u32);
                Ok(Value::Integer(result))
            } else {
                // Negative integer exponent - return float
                let result = (base as f64).powf(*exp as f64);
                Ok(Value::Float(result))
            }
        }
        Value::Float(exp) => {
            // Float exponent - return float
            let result = (base as f64).powf(*exp);
            Ok(Value::Float(result))
        }
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_and(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap();

    match (&lhs, rhs) {
        (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l & r)),
        (Value::BigInt(l), Value::BigInt(r)) => Ok(Value::BigInt(Rc::new(l.as_ref() & r.as_ref()))),
        (Value::BigInt(l), Value::Integer(r)) => {
            Ok(Value::BigInt(Rc::new(l.as_ref() & BigInt::from(*r))))
        }
        (Value::Integer(l), Value::BigInt(r)) => {
            Ok(Value::BigInt(Rc::new(BigInt::from(*l) & r.as_ref())))
        }
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_or(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap();

    match (&lhs, rhs) {
        (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l | r)),
        (Value::BigInt(l), Value::BigInt(r)) => Ok(Value::BigInt(Rc::new(l.as_ref() | r.as_ref()))),
        (Value::BigInt(l), Value::Integer(r)) => {
            Ok(Value::BigInt(Rc::new(l.as_ref() | BigInt::from(*r))))
        }
        (Value::Integer(l), Value::BigInt(r)) => {
            Ok(Value::BigInt(Rc::new(BigInt::from(*l) | r.as_ref())))
        }
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_xor(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs = vm.getself()?;
    let rhs = args[0].as_ref().unwrap();

    match (&lhs, rhs) {
        (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l ^ r)),
        (Value::BigInt(l), Value::BigInt(r)) => Ok(Value::BigInt(Rc::new(l.as_ref() ^ r.as_ref()))),
        (Value::BigInt(l), Value::Integer(r)) => {
            Ok(Value::BigInt(Rc::new(l.as_ref() ^ BigInt::from(*r))))
        }
        (Value::Integer(l), Value::BigInt(r)) => {
            Ok(Value::BigInt(Rc::new(BigInt::from(*l) ^ r.as_ref())))
        }
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_not(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match &this {
        Value::Integer(n) => Ok(Value::Integer(!n)),
        Value::BigInt(b) => Ok(Value::BigInt(Rc::new(!b.as_ref()))),
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_lshift(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs: i64 = vm.getself()?.try_into()?;
    let rhs: i64 = args[0].as_ref().unwrap().try_into()?;

    if rhs < 0 {
        return Err(Error::ArgumentError("negative shift count".to_string()));
    }

    Ok(Value::Integer(lhs << rhs))
}

fn mrb_integer_rshift(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let lhs: i64 = vm.getself()?.try_into()?;
    let rhs: i64 = args[0].as_ref().unwrap().try_into()?;

    if rhs < 0 {
        return Err(Error::ArgumentError("negative shift count".to_string()));
    }

    Ok(Value::Integer(lhs >> rhs))
}

fn mrb_integer_abs(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match &this {
        Value::Integer(n) => Ok(Value::Integer(n.abs())),
        Value::BigInt(b) => Ok(Value::BigInt(Rc::new(b.as_ref().abs()))),
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_to_i(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    vm.getself()
}

fn mrb_integer_to_f(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match &this {
        Value::Integer(n) => Ok(Value::Float(*n as f64)),
        Value::BigInt(b) => Ok(Value::Float(bigint_to_f64(b))),
        _ => Err(Error::TypeMismatch),
    }
}

fn mrb_integer_chr(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this: i64 = vm.getself()?.try_into()?;

    if !(0..=0x10FFFF).contains(&this) {
        return Err(Error::RangeError(format!("{} out of char range", this)));
    }

    let ch = char::from_u32(this as u32)
        .ok_or_else(|| Error::RangeError(format!("invalid codepoint: {}", this)))?;

    Ok(Value::from_rc(Rc::new(RObject::string(ch.to_string()))))
}

fn mrb_integer_clamp(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    if args.len() < 2 {
        return Err(Error::ArgumentError(format!(
            "wrong number of arguments (given {}, expected 2)",
            args.len()
        )));
    }

    let this: i64 = vm.getself()?.try_into()?;
    let min: i64 = args[0].as_ref().unwrap().try_into()?;
    let max: i64 = args[1].as_ref().unwrap().try_into()?;

    if min > max {
        return Err(Error::ArgumentError(
            "min argument must be smaller than max argument".to_string(),
        ));
    }

    let result = if this < min {
        min
    } else if this > max {
        max
    } else {
        this
    };

    Ok(Value::Integer(result))
}
