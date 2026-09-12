// Range methods missing from the mrubyedge prelude.

use crate::Error;
use crate::yamrb::helpers::mrb_define_cmethod;
use crate::yamrb::value::{RObject, RValue, Value};
use crate::yamrb::vm::VM;

use super::{BlockResult, call_block_catch_break, need_block};
use crate::compat::util::{arg_f64, arg_i64, class_rc, native};

type Parts = (f64, f64, bool);

fn range_parts(vm: &mut VM) -> Result<Parts, Error> {
    let this = vm.getself()?;
    match this.rvalue() {
        Some(RValue::Range(s, e, excl)) => {
            let sv = number(s)?;
            let ev = number(e)?;
            Ok((sv, ev, *excl))
        }
        _ => Err(Error::RuntimeError("receiver is not a Range".into())),
    }
}

fn number(o: &Value) -> Result<f64, Error> {
    f64::try_from(o).map_err(|_| Error::RuntimeError("range bounds must be numeric".into()))
}

fn int_out(v: i64) -> Value {
    Value::Integer(v)
}

// Range#begin / #first.
fn cm_range_begin(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match this.rvalue() {
        Some(RValue::Range(s, _, _)) => Ok(s.clone()),
        _ => Err(Error::RuntimeError("receiver is not a Range".into())),
    }
}

// Range#end / #last.
fn cm_range_end(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match this.rvalue() {
        Some(RValue::Range(_, e, _)) => Ok(e.clone()),
        _ => Err(Error::RuntimeError("receiver is not a Range".into())),
    }
}

// Range#exclude_end?.
fn cm_range_exclude_end(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let (_, _, excl) = range_parts(vm)?;
    Ok(Value::Bool(excl))
}

// Range#cover?(v) and Range#===(v) for case/when dispatch.
fn cm_range_cover(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let (start, end, excl) = range_parts(vm)?;
    let probe = match args.first().and_then(|a| a.as_ref()) {
        Some(a) => match a {
            Value::Integer(_) | Value::Float(_) => number(a)?,
            _ => return Ok(Value::Bool(false)),
        },
        None => return Err(Error::ArgumentError("cover? requires an argument".into())),
    };
    let above = probe >= start;
    let below = if excl { probe < end } else { probe <= end };
    Ok(Value::Bool(above && below))
}

// Range#size: element count for integer ranges.
fn cm_range_size(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let (start, end, excl) = range_parts(vm)?;
    if !args.is_empty()
        && !matches!(
            args[0].as_ref().unwrap(),
            Value::Object(o) if matches!(&o.value, RValue::Proc(_))
        )
    {
        let by = arg_f64(args, 0)?;
        if by <= 0.0 {
            return Err(Error::ArgumentError("step must be positive".into()));
        }
        let span = if excl { end - start } else { end - start + 1.0 };
        return Ok(Value::Integer((span / by).ceil() as i64));
    }
    let count = if excl {
        (end - start).max(0.0)
    } else {
        (end - start + 1.0).max(0.0)
    };
    Ok(Value::Integer(count as i64))
}

// Range#first(n = nil).
fn cm_range_first(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let (start, end, excl) = range_parts(vm)?;
    match args.first().and_then(|a| a.as_ref()) {
        Some(c)
            if !matches!(
                c,
                Value::Object(o) if matches!(&o.value, RValue::Proc(_))
            ) =>
        {
            let n = arg_i64(args, 0)?;
            if n < 0 {
                return Err(Error::ArgumentError("negative array size".into()));
            }
            let mut out = Vec::new();
            let mut cur = start as i64;
            while out.len() < n as usize
                && (if excl {
                    cur < end as i64
                } else {
                    cur <= end as i64
                })
            {
                out.push(int_out(cur));
                match cur.checked_add(1) {
                    Some(next) => cur = next,
                    None => break,
                }
            }
            Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
        }
        _ => match this.rvalue() {
            Some(RValue::Range(s, _, _)) => Ok(s.clone()),
            _ => unreachable!(),
        },
    }
}

// Range#last(n = nil).
fn cm_range_last(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match (this.rvalue(), args.first().and_then(|a| a.as_ref())) {
        (Some(RValue::Range(s, e, excl)), Some(c))
            if !matches!(
                c,
                Value::Object(o) if matches!(&o.value, RValue::Proc(_))
            ) =>
        {
            let n = arg_i64(args, 0)?;
            if n < 0 {
                return Err(Error::ArgumentError("negative array size".into()));
            }
            let (Value::Integer(sv), Value::Integer(ev)) = (s, e) else {
                return Err(Error::ArgumentError(
                    "cannot take last(n) of a non-integer range".into(),
                ));
            };
            let (sv, ev, excl) = (*sv, *ev, *excl);
            // Exclusive end means the last yielded value is end - 1.
            let Some(eff_end) = (if excl { ev.checked_sub(1) } else { Some(ev) }) else {
                return Ok(Value::from_rc(
                    RObject::array(Vec::new()).to_refcount_assigned(),
                ));
            };
            if eff_end < sv {
                return Ok(Value::from_rc(
                    RObject::array(Vec::new()).to_refcount_assigned(),
                ));
            }
            let len = eff_end as i128 - sv as i128 + 1;
            let count = (n as i128).min(len).max(0) as i64;
            if count == 0 {
                return Ok(Value::from_rc(
                    RObject::array(Vec::new()).to_refcount_assigned(),
                ));
            }
            let first = eff_end - count + 1;
            let mut out = Vec::new();
            let mut cur = first;
            while cur <= eff_end {
                out.push(int_out(cur));
                match cur.checked_add(1) {
                    Some(next) => cur = next,
                    None => break,
                }
            }
            Ok(Value::from_rc(RObject::array(out).to_refcount_assigned()))
        }
        (Some(RValue::Range(_, e, _)), _) => Ok(e.clone()),
        _ => unreachable!(),
    }
}

// Range#step(n) { |i| }: walks the range in fixed increments.
fn cm_range_step(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = need_block(args)?;
    let positional_len = if matches!(
        args.last().and_then(|a| a.as_ref()),
        Some(Value::Object(o)) if matches!(&o.value, RValue::Proc(_))
    ) {
        args.len() - 1
    } else {
        args.len()
    };
    let step = if positional_len > 0 {
        arg_i64(args, 0)?
    } else {
        1
    };
    if step <= 0 {
        return Err(Error::ArgumentError("step must be positive".into()));
    }
    let (start, end, excl) = range_parts(vm)?;
    let mut cur = start as i64;
    while if excl {
        cur < end as i64
    } else {
        cur <= end as i64
    } {
        match call_block_catch_break(vm, &block, std::slice::from_ref(&int_out(cur)))? {
            BlockResult::Broke(v) => return Ok(v),
            BlockResult::Value(_) => {}
        }
        match cur.checked_add(step) {
            Some(next) => cur = next,
            None => break,
        }
    }
    vm.getself()
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let range = class_rc(vm, "Range")?;
    native!(mrb_define_cmethod, vm, range, "begin", cm_range_begin);
    native!(mrb_define_cmethod, vm, range, "first", cm_range_first);
    native!(mrb_define_cmethod, vm, range, "end", cm_range_end);
    native!(mrb_define_cmethod, vm, range, "last", cm_range_last);
    native!(
        mrb_define_cmethod,
        vm,
        range,
        "exclude_end?",
        cm_range_exclude_end
    );
    native!(mrb_define_cmethod, vm, range, "cover?", cm_range_cover);
    native!(mrb_define_cmethod, vm, range, "===", cm_range_cover);
    native!(mrb_define_cmethod, vm, range, "size", cm_range_size);
    native!(mrb_define_cmethod, vm, range, "step", cm_range_step);
    Ok(())
}
