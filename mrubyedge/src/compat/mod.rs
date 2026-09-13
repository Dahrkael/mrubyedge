// Ruby stdlib compatibility layer, implemented on top of the public VM API.
// One file per Ruby class. Enable with the `ruby-compat` feature and call
// [`register`] after creating a VM.

mod array;
mod comparable;
mod enumerable;
mod exceptions;
mod hash;
mod math;
mod numeric;
mod object_ext;
mod proc_ext;
mod range_extras;
mod string;
mod symbol_ext;
mod util;

#[cfg(test)]
mod tests;

use std::rc::Rc;

use crate::yamrb::helpers::{mrb_call_block, mrb_funcall};
use crate::yamrb::value::{RObject, RValue, Value};
use crate::{Error, yamrb::vm::VM};

/// Outcome of running a user block from a compat iterator.
pub(crate) enum BlockResult {
    /// The block returned normally with this value.
    Value(Value),
    /// The block executed `break v`; the iterator must stop and return v.
    Broke(Value),
}

/// Runs a block like a prelude iterator, translating `break v` into
/// `BlockResult::Broke` instead of letting the Break error escape the native
/// method. Consumes the pending `_Break` exception, mirroring `each`/`times`.
pub(crate) fn call_block_catch_break(
    vm: &mut VM,
    block: &Rc<RObject>,
    args: &[Value],
) -> Result<BlockResult, Error> {
    match mrb_call_block(vm, block.clone(), None, args, 0) {
        Ok(v) => Ok(BlockResult::Value(v)),
        Err(Error::Break(v)) => {
            vm.exception.take();
            Ok(BlockResult::Broke(v))
        }
        Err(e) => Err(e),
    }
}

/// The caller's block rides as the trailing argument in this VM, as a Proc.
pub(crate) fn block_of(args: &[Option<Value>]) -> Option<Rc<RObject>> {
    args.last().and_then(|a| a.as_ref()).and_then(|a| match a {
        Value::Object(o) if matches!(&o.value, RValue::Proc(_)) => Some(o.clone()),
        _ => None,
    })
}

/// Positional-only view of the arguments (trailing block stripped).
pub(crate) fn pos_args(args: &[Option<Value>]) -> &[Option<Value>] {
    match block_of(args) {
        Some(_) => &args[..args.len() - 1],
        None => args,
    }
}

/// The trailing block, or an ArgumentError when none was given.
pub(crate) fn need_block(args: &[Option<Value>]) -> Result<Rc<RObject>, Error> {
    block_of(args).ok_or_else(|| Error::ArgumentError("no block given".into()))
}

/// Equality through the element's own `==` (honors custom overrides).
pub(crate) fn rb_eq(vm: &mut VM, a: &Value, b: &Value) -> Result<bool, Error> {
    let res = mrb_funcall(vm, Some(a.clone()), "==", std::slice::from_ref(b))?;
    Ok(res.is_truthy())
}

/// Installs every compat method into an already-built VM. Safe to call once
/// per VM after the prelude has run.
pub fn register(vm: &mut VM) -> Result<(), Error> {
    math::register(vm)?;
    array::register(vm)?;
    comparable::register(vm)?;
    enumerable::register(vm)?;
    hash::register(vm)?;
    string::register(vm)?;
    numeric::register(vm)?;
    symbol_ext::register(vm)?;
    object_ext::register(vm)?;
    exceptions::register(vm)?;
    proc_ext::register(vm)?;
    range_extras::register(vm)?;
    Ok(())
}
