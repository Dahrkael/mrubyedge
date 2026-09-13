// Symbol methods missing from the mrubyedge prelude, built by round-tripping
// through String.

use std::rc::Rc;

use crate::Error;
use crate::compat::util::mrb_funcall;
use crate::yamrb::helpers::mrb_define_cmethod;
use crate::yamrb::value::{RObject, RValue, Value};
use crate::yamrb::vm::VM;

use crate::compat::util::{class_rc, native};

fn sym_string(vm: &mut VM) -> Result<String, Error> {
    let this = vm.getself()?;
    let s = mrb_funcall(vm, Some(this), "to_s", &[])?;
    String::try_from(&s).map_err(|_| Error::RuntimeError("Symbol#to_s failed".into()))
}

fn map_to_symbol(vm: &mut VM, method: &str, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = sym_string(vm)?;
    let text = Value::from_rc(Rc::new(RObject::string(s)));
    let rc_args: Vec<Value> = args.iter().map(|a| a.as_ref().unwrap().clone()).collect();
    let out = mrb_funcall(vm, Some(text), method, &rc_args)?;
    mrb_funcall(vm, Some(out), "to_sym", &[])
}

fn cm_symbol_length(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Integer(sym_string(vm)?.chars().count() as i64))
}

fn cm_symbol_upcase(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    map_to_symbol(vm, "upcase", args)
}

fn cm_symbol_downcase(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    map_to_symbol(vm, "downcase", args)
}

fn cm_symbol_capitalize(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    map_to_symbol(vm, "capitalize", args)
}

fn cm_symbol_swapcase(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    map_to_symbol(vm, "swapcase", args)
}

fn cm_symbol_succ(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    map_to_symbol(vm, "succ", args)
}

fn cm_symbol_empty(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Bool(sym_string(vm)?.is_empty()))
}

// Symbol#[](idx_or_range): slices return Strings like CRuby.
fn cm_symbol_aref(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let s = sym_string(vm)?;
    let text = Value::from_rc(Rc::new(RObject::string(s)));
    let rc_args: Vec<Value> = args.iter().map(|a| a.as_ref().unwrap().clone()).collect();
    mrb_funcall(vm, Some(text), "[]", &rc_args)
}

// Symbol#<=>: nil unless the other side is a Symbol.
fn cm_symbol_spaceship(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let is_sym = matches!(
        args.first().and_then(|v| v.as_ref()),
        Some(Value::Symbol(_))
    );
    if !is_sym {
        return Ok(Value::Nil);
    }
    let a = sym_string(vm)?;
    let other_obj = args.first().and_then(|v| v.as_ref()).unwrap().clone();
    let b = mrb_funcall(vm, Some(other_obj), "to_s", &[])?;
    let b = String::try_from(&b).map_err(|_| Error::RuntimeError("bad symbol".into()))?;
    Ok(Value::Integer(a.cmp(&b) as i64))
}

// Symbol#casecmp / #casecmp?.
fn cm_symbol_casecmp(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let a = sym_string(vm)?;
    if !matches!(
        args.first().and_then(|x| x.as_ref()),
        Some(Value::Symbol(_))
    ) {
        return Ok(Value::Nil);
    }
    let other = args.first().and_then(|x| x.as_ref()).unwrap().clone();
    let b = mrb_funcall(vm, Some(other), "to_s", &[])?;
    let b = String::try_from(&b).map_err(|_| Error::RuntimeError("bad symbol".into()))?;
    Ok(Value::Integer(
        a.to_lowercase().cmp(&b.to_lowercase()) as i64
    ))
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let symbol = class_rc(vm, "Symbol")?;

    // Comparable gives between?/clamp to symbols.
    if let Some(comparable) = vm.get_const_by_name("Comparable")
        && let RValue::Module(m) = &comparable.value
    {
        crate::yamrb::prelude::module::mrb_include_module(&symbol, m.clone())?;
    }

    native!(mrb_define_cmethod, vm, symbol, "length", cm_symbol_length);
    native!(mrb_define_cmethod, vm, symbol, "size", cm_symbol_length);
    native!(mrb_define_cmethod, vm, symbol, "upcase", cm_symbol_upcase);
    native!(
        mrb_define_cmethod,
        vm,
        symbol,
        "downcase",
        cm_symbol_downcase
    );
    native!(
        mrb_define_cmethod,
        vm,
        symbol,
        "capitalize",
        cm_symbol_capitalize
    );
    native!(
        mrb_define_cmethod,
        vm,
        symbol,
        "swapcase",
        cm_symbol_swapcase
    );
    native!(mrb_define_cmethod, vm, symbol, "succ", cm_symbol_succ);
    native!(mrb_define_cmethod, vm, symbol, "empty?", cm_symbol_empty);
    native!(mrb_define_cmethod, vm, symbol, "[]", cm_symbol_aref);
    native!(mrb_define_cmethod, vm, symbol, "<=>", cm_symbol_spaceship);
    native!(mrb_define_cmethod, vm, symbol, "casecmp", cm_symbol_casecmp);
    Ok(())
}
