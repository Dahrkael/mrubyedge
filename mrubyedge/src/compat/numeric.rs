// Integer and Float methods missing from the mrubyedge prelude.

use crate::Error;
use crate::yamrb::helpers::{mrb_call_block, mrb_define_cmethod, mrb_define_cmethod_fast};
use crate::yamrb::value::{FastOp, RObject, RValue, Value, rubylike_mod_f64, rubylike_mod_i64};
use crate::yamrb::vm::VM;

use super::need_block;
use crate::compat::util::{arg_f64, arg_i64, class_rc, native, native_fast};

fn int_of(vm: &mut VM) -> Result<i64, Error> {
    let this = vm.getself()?;
    match &this {
        Value::Integer(n) => Ok(*n),
        _ => Err(Error::RuntimeError("receiver is not an Integer".into())),
    }
}

fn float_of(vm: &mut VM) -> Result<f64, Error> {
    let this = vm.getself()?;
    f64::try_from(&this).map_err(|_| Error::RuntimeError("receiver is not a number".into()))
}

// Integer#upto(n) { |i| }: inclusive ascending walk.
fn cm_integer_upto(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let start = int_of(vm)?;
    let limit = arg_i64(args, 0)?;
    for i in start..=limit {
        let a = Value::Integer(i);
        mrb_call_block(vm, block.clone(), None, std::slice::from_ref(&a), 0)?;
    }
    vm.getself()
}

// Integer#downto(n) { |i| }: inclusive descending walk.
fn cm_integer_downto(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let start = int_of(vm)?;
    let limit = arg_i64(args, 0)?;
    // Reverse range avoids underflowing at i64::MIN.
    for i in (limit..=start).rev() {
        let a = Value::Integer(i);
        mrb_call_block(vm, block.clone(), None, std::slice::from_ref(&a), 0)?;
    }
    vm.getself()
}

// Numeric#step(limit, step = 1): arithmetic walk in either direction.
fn cm_numeric_step(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let positional_len = args.len() - 1; // block rides last
    let limit = arg_f64(args, 0)?;
    let step = if positional_len > 1 {
        arg_f64(args, 1)?
    } else {
        1.0
    };
    if step == 0.0 {
        return Err(Error::ArgumentError("step cannot be zero".into()));
    }
    let mut cur = float_of(vm)?;
    while (step > 0.0 && cur <= limit) || (step < 0.0 && cur >= limit) {
        let a = if is_float_receiver(vm)? {
            Value::Float(cur)
        } else if cur.fract() == 0.0 && cur.abs() < i64::MAX as f64 {
            Value::Integer(cur as i64)
        } else {
            Value::Float(cur)
        };
        mrb_call_block(vm, block.clone(), None, std::slice::from_ref(&a), 0)?;
        cur += step;
    }
    vm.getself()
}

fn is_float_receiver(vm: &mut VM) -> Result<bool, Error> {
    let this = vm.getself()?;
    Ok(matches!(this, Value::Float(_)))
}

// Integer#even? / #odd?.
fn cm_integer_even(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Bool(int_of(vm)? % 2 == 0))
}

fn cm_integer_odd(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Bool(int_of(vm)? % 2 != 0))
}

fn cm_integer_zero(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    sign_predicates(vm, 0)
}

fn cm_integer_positive(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    sign_predicates(vm, 1)
}

fn cm_integer_negative(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    sign_predicates(vm, 2)
}

// Numeric sign predicates shared by Integer and Float receivers.
fn sign_predicates(vm: &mut VM, want: u8) -> Result<Value, Error> {
    let v = float_of(vm)?;
    let hit = match want {
        0 => v == 0.0,
        1 => v > 0.0,
        _ => v < 0.0,
    };
    Ok(Value::Bool(hit))
}

// Integer#succ / #pred.
fn cm_integer_succ(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let n = int_of(vm)?;
    n.checked_add(1)
        .map(Value::Integer)
        .ok_or_else(|| Error::RangeError("integer overflow".into()))
}

fn cm_integer_pred(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let n = int_of(vm)?;
    n.checked_sub(1)
        .map(Value::Integer)
        .ok_or_else(|| Error::RangeError("integer overflow".into()))
}

// Integer#divmod(n): [quotient, modulus] following Ruby's floored division.
fn cm_integer_divmod(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = int_of(vm)?;
    let b = arg_i64(args, 0)?;
    if b == 0 {
        return Err(Error::ZeroDivisionError);
    }
    // i64::MIN / -1 overflows; the true quotient has no i64 representation.
    let (q, r) = if b == -1 {
        (
            a.checked_neg()
                .ok_or_else(|| Error::RangeError("integer overflow".into()))?,
            0,
        )
    } else {
        // The floored quotient fits in i64 for every other divisor; compute the
        // intermediate in i128 so `a - r` cannot overflow.
        let r = rubylike_mod_i64(a, b);
        let q = ((a as i128 - r as i128) / b as i128) as i64;
        (q, r)
    };
    Ok(Value::from_rc(
        RObject::array(vec![Value::Integer(q), Value::Integer(r)]).to_refcount_assigned(),
    ))
}

// Integer#% / #modulo(n): Ruby's floored modulo, result carries the divisor's
// sign. Overrides the prelude's truncated % (Rust % keeps the dividend's).
// Uses the shared `rubylike_mod_*` helpers from the vendored VM so the native
// and the send fast path cannot drift apart.
fn cm_integer_mod(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = int_of(vm)?;
    let Some(first) = args.first().and_then(|v| v.as_ref()) else {
        return Err(Error::ArgumentError("missing argument at arg 0".into()));
    };
    match first {
        Value::Float(b) => {
            if *b == 0.0 {
                return Err(Error::ZeroDivisionError);
            }
            Ok(Value::Float(rubylike_mod_f64(a as f64, *b)))
        }
        _ => {
            let b = arg_i64(args, 0)?;
            if b == 0 {
                return Err(Error::ZeroDivisionError);
            }
            // i64::MIN % -1 overflows inside the shared helper; the result is 0.
            if b == -1 {
                return Ok(Value::Integer(0));
            }
            Ok(Value::Integer(rubylike_mod_i64(a, b)))
        }
    }
}

// Float#% / #modulo(n): floored modulo, matching Integer#%.
fn cm_float_mod(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = float_of(vm)?;
    let b = arg_f64(args, 0)?;
    if b == 0.0 {
        return Err(Error::ZeroDivisionError);
    }
    Ok(Value::Float(rubylike_mod_f64(a, b)))
}

// Float#divmod(n): [floored quotient, modulo] as floats.
fn cm_float_divmod(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = float_of(vm)?;
    let b = arg_f64(args, 0)?;
    if b == 0.0 {
        return Err(Error::ZeroDivisionError);
    }
    let q = (a / b).floor();
    Ok(Value::from_rc(
        RObject::array(vec![Value::Float(q), Value::Float(a - b * q)]).to_refcount_assigned(),
    ))
}

// Float#remainder(n): truncated remainder, carries the dividend's sign.
fn cm_float_remainder(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = float_of(vm)?;
    let b = arg_f64(args, 0)?;
    if b == 0.0 {
        return Err(Error::ZeroDivisionError);
    }
    Ok(Value::Float(a % b))
}

// Integer#fdiv(n).
fn cm_integer_fdiv(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = int_of(vm)?;
    Ok(Value::Float(a as f64 / arg_f64(args, 0)?))
}

// Integer#div(n): floored integer quotient.
fn cm_integer_div(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let b = arg_i64(args, 0)?;
    if b == 0 {
        return Err(Error::ZeroDivisionError);
    }
    Ok(Value::Integer(int_of(vm)?.div_euclid(b)))
}

// Integer#pow(e, mod = nil): modular exponentiation when mod is given.
fn cm_integer_pow(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let base = int_of(vm)?;
    let e = arg_i64(args, 0)?;
    if e < 0 {
        return Err(Error::ArgumentError("negative exponent".into()));
    }
    if args.len() > 1 {
        let m = arg_i64(args, 1)?;
        if m == 0 {
            return Err(Error::ZeroDivisionError);
        }
        let mut result: i128 = 1;
        let mut b: i128 = base as i128;
        let mut exp = e;
        let modulus = m as i128;
        while exp > 0 {
            if exp % 2 == 1 {
                result = result * b % modulus;
            }
            b = b * b % modulus;
            exp /= 2;
        }
        return Ok(Value::Integer(result as i64));
    }
    let mut acc: i64 = 1;
    for _ in 0..e {
        acc = acc.saturating_mul(base);
    }
    Ok(Value::Integer(acc))
}

// Integer#digits(base = 10): least significant digit first.
fn cm_integer_digits(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let n = int_of(vm)?;
    if n < 0 {
        return Err(Error::ArgumentError("negative number".into()));
    }
    let base = if args.is_empty()
        || matches!(
            args[0].as_ref().unwrap(),
            Value::Object(o) if matches!(&o.value, RValue::Proc(_))
        ) {
        10
    } else {
        arg_i64(args, 0)?
    };
    if base <= 1 {
        return Err(Error::ArgumentError("invalid radix".into()));
    }
    let mut out = Vec::new();
    let mut cur = n;
    loop {
        out.push(Value::Integer(cur % base));
        cur /= base;
        if cur == 0 {
            break;
        }
    }
    Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
}

// Greatest common divisor over magnitudes (i64::MIN.abs() would overflow).
fn gcd_mag(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

fn cm_integer_gcd(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let g = gcd_mag(int_of(vm)?.unsigned_abs(), arg_i64(args, 0)?.unsigned_abs());
    if g > i64::MAX as u64 {
        return Err(Error::RangeError("gcd overflow".into()));
    }
    Ok(Value::Integer(g as i64))
}

fn cm_integer_lcm(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = int_of(vm)?.unsigned_abs();
    let b = arg_i64(args, 0)?.unsigned_abs();
    let g = gcd_mag(a, b);
    if g == 0 {
        return Ok(Value::Integer(0));
    }
    let l = (a / g) as i128 * b as i128;
    if l > i64::MAX as i128 {
        return Err(Error::RangeError("lcm overflow".into()));
    }
    Ok(Value::Integer(l as i64))
}

// Integer#bit_length.
fn cm_integer_bit_length(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let n = int_of(vm)?;
    let magnitude = if n < 0 { -(n + 1) } else { n };
    let bits = if magnitude == 0 {
        0
    } else {
        64 - (magnitude as u64).leading_zeros() as i64
    };
    Ok(Value::Integer(bits))
}

// Numeric#coerce(other): both sides as floats.
fn cm_numeric_coerce(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let other = arg_f64(args, 0)?;
    let me = float_of(vm)?;
    Ok(Value::from_rc(
        RObject::array(vec![Value::Float(other), Value::Float(me)]).to_refcount_assigned(),
    ))
}

/// Shared rounding family for Float receivers with an optional digit count.
/// Positive digits return a Float; zero/negative digits return an Integer,
/// matching Ruby (e.g. `15.5.round(-1) == 20`).
fn float_round_mode(vm: &mut VM, args: &[Option<Value>], mode: u8) -> Result<Value, Error> {
    let v = float_of(vm)?;
    let digits = if args.is_empty()
        || matches!(
            args[0].as_ref().unwrap(),
            Value::Object(o) if matches!(&o.value, RValue::Proc(_))
        ) {
        0
    } else {
        arg_i64(args, 0)?
    };
    // Clamp to the f64 powi range; magnitudes beyond it saturate to inf/0.
    let digits = digits.clamp(-308, 308) as i32;
    let round = |scaled: f64| -> f64 {
        match mode {
            0 => {
                // Ruby rounds half away from zero.
                if scaled >= 0.0 {
                    scaled.floor() + if scaled.fract() >= 0.5 { 1.0 } else { 0.0 }
                } else {
                    scaled.ceil() - if (-scaled).fract() >= 0.5 { 1.0 } else { 0.0 }
                }
            }
            1 => scaled.floor(),
            2 => scaled.ceil(),
            _ => scaled.trunc(),
        }
    };
    if digits <= 0 {
        let factor = 10f64.powi(-digits);
        let shifted = round(v / factor);
        Ok(Value::Integer((shifted * factor) as i64))
    } else {
        let factor = 10f64.powi(digits);
        let shifted = round(v * factor);
        Ok(Value::Float(shifted / factor))
    }
}

fn cm_float_round(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    float_round_mode(vm, args, 0)
}

fn cm_float_floor(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    float_round_mode(vm, args, 1)
}

fn cm_float_ceil(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    float_round_mode(vm, args, 2)
}

fn cm_float_truncate(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    float_round_mode(vm, args, 3)
}

// Integer#round(n = 0): n <= 0 keeps integers; n > 0 returns self unchanged.
fn cm_integer_round(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let digits = if args.is_empty()
        || matches!(
            args[0].as_ref().unwrap(),
            Value::Object(o) if matches!(&o.value, RValue::Proc(_))
        ) {
        0
    } else {
        arg_i64(args, 0)?
    };
    Ok(match digits {
        d if d < 0 => Value::Integer(
            (int_of(vm)? as f64 / 10f64.powi(-d as i32)).round() as i64
                * 10f64.powi(-d as i32) as i64,
        ),
        _ => this,
    })
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let integer = class_rc(vm, "Integer")?;
    native!(mrb_define_cmethod, vm, integer, "upto", cm_integer_upto);
    native!(mrb_define_cmethod, vm, integer, "downto", cm_integer_downto);
    native!(mrb_define_cmethod, vm, integer, "step", cm_numeric_step);
    native!(mrb_define_cmethod, vm, integer, "even?", cm_integer_even);
    native!(mrb_define_cmethod, vm, integer, "odd?", cm_integer_odd);
    native!(mrb_define_cmethod, vm, integer, "zero?", cm_integer_zero);
    native!(
        mrb_define_cmethod,
        vm,
        integer,
        "positive?",
        cm_integer_positive
    );
    native!(
        mrb_define_cmethod,
        vm,
        integer,
        "negative?",
        cm_integer_negative
    );
    native!(mrb_define_cmethod, vm, integer, "succ", cm_integer_succ);
    native!(mrb_define_cmethod, vm, integer, "pred", cm_integer_pred);
    native!(mrb_define_cmethod, vm, integer, "divmod", cm_integer_divmod);
    native_fast!(
        mrb_define_cmethod_fast,
        vm,
        integer,
        "%",
        FastOp::IntModFloored,
        cm_integer_mod
    );
    native_fast!(
        mrb_define_cmethod_fast,
        vm,
        integer,
        "modulo",
        FastOp::IntModFloored,
        cm_integer_mod
    );
    native!(mrb_define_cmethod, vm, integer, "fdiv", cm_integer_fdiv);
    native!(mrb_define_cmethod, vm, integer, "div", cm_integer_div);
    native!(mrb_define_cmethod, vm, integer, "pow", cm_integer_pow);
    native!(mrb_define_cmethod, vm, integer, "digits", cm_integer_digits);
    native!(mrb_define_cmethod, vm, integer, "gcd", cm_integer_gcd);
    native!(mrb_define_cmethod, vm, integer, "lcm", cm_integer_lcm);
    native!(
        mrb_define_cmethod,
        vm,
        integer,
        "bit_length",
        cm_integer_bit_length
    );
    native!(mrb_define_cmethod, vm, integer, "coerce", cm_numeric_coerce);
    native!(mrb_define_cmethod, vm, integer, "round", cm_integer_round);

    let float = class_rc(vm, "Float")?;
    native!(mrb_define_cmethod, vm, float, "step", cm_numeric_step);
    native!(mrb_define_cmethod, vm, float, "coerce", cm_numeric_coerce);
    native_fast!(
        mrb_define_cmethod_fast,
        vm,
        float,
        "%",
        FastOp::FloatModFloored,
        cm_float_mod
    );
    native_fast!(
        mrb_define_cmethod_fast,
        vm,
        float,
        "modulo",
        FastOp::FloatModFloored,
        cm_float_mod
    );
    native!(mrb_define_cmethod, vm, float, "divmod", cm_float_divmod);
    native!(
        mrb_define_cmethod,
        vm,
        float,
        "remainder",
        cm_float_remainder
    );
    native!(mrb_define_cmethod, vm, float, "round", cm_float_round);
    native!(mrb_define_cmethod, vm, float, "floor", cm_float_floor);
    native!(mrb_define_cmethod, vm, float, "ceil", cm_float_ceil);
    native!(mrb_define_cmethod, vm, float, "truncate", cm_float_truncate);
    Ok(())
}
