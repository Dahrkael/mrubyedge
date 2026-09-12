use crate::{
    Error,
    yamrb::{
        helpers::{mrb_call_block, mrb_define_cmethod},
        prelude::module::mrb_include_module,
        value::{RValue, Value},
        vm::VM,
    },
};

pub(crate) fn initialize_range(vm: &mut VM) {
    let range_class = vm.define_standard_class("Range");

    mrb_define_cmethod(
        vm,
        range_class.clone(),
        "include?",
        Box::new(mrb_range_is_include),
    );
    mrb_define_cmethod(vm, range_class.clone(), "each", Box::new(mrb_range_each));

    let enumerable_module = vm.get_module_by_name("Enumerable");
    mrb_include_module(&range_class, enumerable_module).expect("failed to include Enumerable");
}

pub fn mrb_range_is_include(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    match &this.value {
        RValue::Range(start, end, exclusive) => {
            let obj = args[0].as_ref().unwrap().clone();
            match (start, end, &obj) {
                (Value::Integer(s), Value::Integer(e), Value::Integer(o)) => {
                    let (s, e, o) = (*s, *e, *o);
                    if *exclusive {
                        Ok(Value::Bool(s <= o && o < e))
                    } else {
                        Ok(Value::Bool(s <= o && o <= e))
                    }
                }
                (Value::Integer(s), Value::Integer(e), Value::Float(o)) => {
                    let o = *o as i64;
                    let (s, e) = (*s, *e);
                    if *exclusive {
                        Ok(Value::Bool(s <= o && o < e))
                    } else {
                        Ok(Value::Bool(s <= o && o <= e))
                    }
                }
                _ => Ok(Value::Bool(false)),
            }
        }
        _ => Err(Error::RuntimeError(
            "Range#include? must be called on a Range".to_string(),
        )),
    }
}

pub fn mrb_range_each(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let this = vm.getself()?;
    let block = args[0].as_ref().unwrap().clone();
    match &this.value {
        RValue::Range(start, end, exclusive) => match (start, end) {
            (Value::Integer(start), Value::Integer(end)) => {
                let start = *start;
                let mut end = *end;
                if *exclusive {
                    end -= 1;
                }
                for i in start..=end {
                    let args = vec![Value::Integer(i)];
                    match mrb_call_block(vm, block.to_rc(), None, &args, 0) {
                        Ok(_) => {}
                        // break inside the block stops each
                        // and its value becomes the result (Ruby semantics).
                        // Consume the pending exception (see integer.rs note).
                        Err(Error::Break(v)) => {
                            vm.exception.take();
                            return Ok(Value::from_rc(v));
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            _ => {
                return Err(Error::RuntimeError(
                    "Range#each must be called on a integer Range with block (for now)".to_string(),
                ));
            }
        },
        _ => {
            return Err(Error::RuntimeError(
                "Range#each must be called on a Range".to_string(),
            ));
        }
    }
    Ok(Value::from_rc(this.clone()))
}
