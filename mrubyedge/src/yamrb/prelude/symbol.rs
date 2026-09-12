use std::rc::Rc;

use crate::Error;
use crate::yamrb::helpers::{mrb_define_cmethod, mrb_funcall};

use crate::yamrb::{
    value::{RFn, RObject, RProc, Value},
    vm::VM,
};

pub(crate) fn initialize_symbol(vm: &mut VM) {
    let symbol_class = vm.define_standard_class("Symbol");
    mrb_define_cmethod(vm, symbol_class.clone(), "to_s", Box::new(mrb_symbol_to_s));
    mrb_define_cmethod(
        vm,
        symbol_class.clone(),
        "inspect",
        Box::new(mrb_symbol_inspect),
    );
    mrb_define_cmethod(
        vm,
        symbol_class.clone(),
        "to_proc",
        Box::new(mrb_symbol_to_proc),
    );
}

fn mrb_symbol_inspect(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let this: String = vm.getself()?.try_into()?;
    Ok(Value::from_rc(Rc::new(RObject::string(format!(
        ":{}",
        this
    )))))
}

fn mrb_symbol_to_s(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let symbol: String = vm.getself()?.try_into()?;
    Ok(Value::from_rc(Rc::new(RObject::string(symbol))))
}

fn mrb_symbol_to_proc(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let method_name: String = vm.getself()?.try_into()?;
    let rfn: RFn = Box::new(move |vm: &mut VM, args: &[Option<Value>]| {
        let recv = args
            .first()
            .and_then(|a| a.as_ref())
            .cloned()
            .ok_or_else(|| Error::ArgumentError("no receiver given".to_string()))?;
        let method_args: Vec<Value> = args[1..]
            .iter()
            .map(|a| a.as_ref().unwrap().clone())
            .collect();
        mrb_funcall(vm, Some(recv), &method_name, &method_args)
    });
    vm.push_fnblock(Rc::new(rfn))?;
    let block = RProc {
        is_rb_func: false,
        is_fnblock: true,
        sym_id: None,
        next: None,
        irep: None,
        func: None,
        environ: None,
        block_self: vm.getself().ok(),
    };
    Ok(Value::from_rc(RObject::proc(block).to_refcount_assigned()))
}
