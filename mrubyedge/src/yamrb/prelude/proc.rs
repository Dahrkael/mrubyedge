use std::rc::Rc;

use crate::{
    Error,
    yamrb::{
        helpers::{mrb_call_block, mrb_define_class_cmethod, mrb_define_cmethod},
        value::*,
        vm::{CallerLabel, VM},
    },
};

pub(crate) fn initialize_proc(vm: &mut VM) {
    let proc_class = vm.define_standard_class("Proc");

    mrb_define_class_cmethod(vm, proc_class.clone(), "new", Box::new(mrb_proc_new));

    mrb_define_cmethod(vm, proc_class.clone(), "call", Box::new(mrb_proc_call));
}

fn mrb_proc_new(_vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let block = args[0].as_ref().unwrap().clone();
    Ok(block)
}

pub fn mrb_proc_call(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    // handle Proc#call as special: replace the send crumb with a Proc#call
    // crumb, keeping its return register so break and unwinding still land.
    let cur = vm
        .breadcrumbs
        .borrow_mut()
        .pop()
        .expect("empty breadcrumb on call");
    vm.push_breadcrumb(
        "_proc_call_via_method",
        Some(CallerLabel::Static("Proc#call")),
        cur.return_reg,
        Some(vm.current_irep.clone()),
        Some(vm.pc.get().saturating_sub(1)),
    );

    let this = vm.getself()?;
    let args: Vec<Rc<RObject>> = args.iter().map(|a| a.as_ref().unwrap().to_rc()).collect();
    Ok(Value::from_rc(mrb_call_block(
        vm,
        this.clone(),
        None,
        &args,
        0,
    )?))
}
