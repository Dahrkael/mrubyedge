use std::rc::Rc;

use crate::{
    Error,
    yamrb::{
        helpers::{mrb_define_cmethod, mrb_define_cmethod_attr, mrb_funcall},
        value::*,
        vm::VM,
    },
};

pub(crate) fn initialize_class(vm: &mut VM) {
    let module_class = vm.get_class_by_name("Module");
    mrb_define_cmethod(
        vm,
        module_class.clone(),
        "inspect",
        Box::new(mrb_module_inspect),
    );

    let class_class = vm.define_standard_class_with_superclass("Class", module_class);

    // Create singleton class for Object class
    RObject::class(vm.object_class.clone(), vm).initialize_or_get_singleton_class_for_class(vm);

    mrb_define_cmethod(vm, class_class.clone(), "new", Box::new(mrb_class_new));
    mrb_define_cmethod(
        vm,
        class_class.clone(),
        "attr_reader",
        Box::new(mrb_class_attr_reader),
    );
    mrb_define_cmethod(
        vm,
        class_class.clone(),
        "attr_writer",
        Box::new(mrb_class_attr_writer),
    );
    mrb_define_cmethod(
        vm,
        class_class.clone(),
        "attr_accessor",
        Box::new(mrb_class_attr_acceccor),
    );
    mrb_define_cmethod(
        vm,
        class_class.clone(),
        "attr",
        Box::new(mrb_class_attr_acceccor),
    );
    mrb_define_cmethod(vm, class_class, "ancestors", Box::new(mrb_class_ancestors));
}

fn mrb_class_new(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let class = vm.getself()?;
    let class = match class.rvalue() {
        Some(RValue::Class(c)) => c.clone(),
        _ => {
            return Err(Error::RuntimeError(
                "Class#new must be called from class".to_string(),
            ));
        }
    };

    let obj = RObject::instance(class).to_refcount_assigned();

    let rc_args: Vec<Value> = args.iter().map(|a| a.as_ref().unwrap().clone()).collect();
    mrb_funcall(
        vm,
        Some(Value::from_rc(obj.clone())),
        "initialize",
        &rc_args,
    )?;

    Ok(Value::from_rc(obj))
}

fn mrb_class_attr_reader(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let class_ = vm.getself()?;
    let class = match class_.rvalue() {
        Some(RValue::Class(c)) => c.clone(),
        _ => {
            return Err(Error::RuntimeError(
                "Class#attr_reader must be called from class".to_string(),
            ));
        }
    };
    for arg in args.iter() {
        match arg.as_ref().unwrap() {
            Value::Symbol(id) => {
                let sym_id: &'static str = symbol_name(*id).leak();
                // Build the ivar key once; reads reuse the symbol id so they
                // never touch a string key or an interning lookup.
                let key = intern_symbol(&format!("@{}", sym_id));
                let method = {
                    move |vm: &mut VM, _args: &[Option<Value>]| {
                        let this = vm.getself()?;
                        Ok(this.get_ivar_by_id(key))
                    }
                };
                mrb_define_cmethod_attr(
                    vm,
                    class.clone(),
                    sym_id,
                    FastOp::AttrGet,
                    key,
                    Box::new(method),
                );
            }
            Value::Nil => {
                // skip
            }
            _ => {
                return Err(Error::RuntimeError(
                    "Class#attr_reader must be called with symbols".to_string(),
                ));
            }
        }
    }
    Ok(Value::Nil)
}

fn mrb_class_attr_writer(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    let class_ = vm.getself()?;
    let class = match class_.rvalue() {
        Some(RValue::Class(c)) => c.clone(),
        _ => {
            return Err(Error::RuntimeError(
                "Class#attr_reader must be called from class".to_string(),
            ));
        }
    };
    for arg in args.iter() {
        match arg.as_ref().unwrap() {
            Value::Symbol(id) => {
                let sym_id: &'static str = symbol_name(*id).leak();
                // Build the ivar key once; writes reuse the symbol id so they
                // never touch a string key or an interning lookup.
                let key = intern_symbol(&format!("@{}", sym_id));
                let method = {
                    move |vm: &mut VM, args: &[Option<Value>]| {
                        let this = vm.getself()?;
                        let value = args[0].as_ref().unwrap().clone();
                        this.set_ivar_by_id(key, value.clone());
                        Ok(value)
                    }
                };
                let method_name = format!("{}=", sym_id);
                mrb_define_cmethod_attr(
                    vm,
                    class.clone(),
                    &method_name,
                    FastOp::AttrSet,
                    key,
                    Box::new(method),
                );
            }
            Value::Nil => {
                // skip
            }
            _ => {
                return Err(Error::RuntimeError(
                    "Class#attr_reader must be called with symbols".to_string(),
                ));
            }
        }
    }
    Ok(Value::Nil)
}

fn mrb_class_attr_acceccor(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
    mrb_class_attr_reader(vm, args)?;
    mrb_class_attr_writer(vm, args)
}

fn mrb_class_ancestors(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let self_module = vm.getself()?;
    let target_class = match self_module.rvalue() {
        Some(RValue::Class(class)) => class.clone(),
        _ => {
            return Err(Error::RuntimeError(
                "Module#ancestors must be called on class or module".to_string(),
            ));
        }
    };
    let ancestors: Vec<Value> = build_lookup_chain(&target_class)
        .iter()
        .map(|m| Value::from_rc(RObject::class_or_module(m.clone(), vm)))
        .collect();
    Ok(Value::from_rc(
        RObject::array(ancestors).to_refcount_assigned(),
    ))
}

fn mrb_module_inspect(vm: &mut VM, _args: &[Option<Value>]) -> Result<Value, Error> {
    let class = vm.getself()?;
    let class_name = match class.rvalue() {
        Some(RValue::Class(c)) => c.full_name(),
        Some(RValue::Module(m)) => m.full_name(),
        _ => {
            return Err(Error::RuntimeError(
                "Module#inspect must be called from module or class".to_string(),
            ));
        }
    };
    Ok(Value::from_rc(Rc::new(RObject::string(class_name))))
}

#[test]
fn test_class_attr_accessor() {
    use crate::yamrb::helpers::*;

    let mut vm = VM::empty();
    let class = vm.define_class("Test", None, None);
    let args = [Some(Value::from_rc(
        RObject::symbol("foo".into()).to_refcount_assigned(),
    ))];
    let classobj = RObject::class(class.clone(), &mut vm);
    vm.set_reg(0, classobj.clone());
    mrb_class_attr_acceccor(&mut vm, &args).expect("mrb_class_attr_acceccor failed");

    let instance = RObject::instance(class).to_refcount_assigned();

    let args = vec![Value::Integer(557188)];
    mrb_funcall(
        &mut vm,
        Some(Value::from_rc(instance.clone())),
        "foo=",
        &args,
    )
    .expect("call obj.foo = failed");

    let ret = mrb_funcall(&mut vm, Some(Value::from_rc(instance.clone())), "foo", &[])
        .expect("call obj.foo failed");
    let ret: i64 = ret
        .to_rc()
        .as_ref()
        .try_into()
        .expect("obj.foo must be integer");
    assert_eq!(ret, 557188);
}
