// Proc methods missing from the mrubyedge prelude.

use crate::Error;
use crate::compat::util::mrb_funcall;
use crate::yamrb::helpers::mrb_define_cmethod;
use crate::yamrb::value::{RValue, Value};
use crate::yamrb::vm::VM;

// Proc#===: case/when dispatch alias of call.
fn cm_proc_call_alias(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let vals: Vec<Value> = args.iter().map(|a| a.as_ref().unwrap().clone()).collect();
    mrb_funcall(vm, Some(this), "call", &vals)
}

// Proc#to_proc: identity.
fn cm_proc_to_proc(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    vm.getself()
}

// Proc#arity: reports -1 (accepts any count); precise arity needs irep
// introspection that the public API does not expose.
fn cm_proc_arity(_vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    Ok(Value::Integer(-1))
}

pub(crate) fn register(vm: &mut VM) -> Result<(), Error> {
    let proc_class = match vm.get_const_by_name("Proc") {
        Some(o) => match &o.value {
            RValue::Class(c) => c.clone(),
            _ => return Err(Error::NameError("Proc".into())),
        },
        None => return Err(Error::NameError("Proc".into())),
    };
    mrb_define_cmethod(vm, proc_class.clone(), "===", Box::new(cm_proc_call_alias));
    mrb_define_cmethod(vm, proc_class.clone(), "to_proc", Box::new(cm_proc_to_proc));
    mrb_define_cmethod(vm, proc_class, "arity", Box::new(cm_proc_arity));
    Ok(())
}
