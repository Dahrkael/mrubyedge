#![allow(unused_imports)]
#![allow(dead_code)]
use std::rc::Rc;

pub use mrubyedge::yamrb::value::{RObject, RSym, Value};

pub use mrubyedge::yamrb::helpers::mrb_funcall;

pub(crate) fn mrbc_compile(_fname: &'static str, code: &'static str) -> Vec<u8> {
    unsafe {
        let mut context = mruby_compiler2_sys::MRubyCompiler2Context::new();
        context.compile(code).unwrap()
    }
}

pub(crate) fn mrbc_compile_debug(_fname: &'static str, code: &'static str) -> Vec<u8> {
    unsafe {
        let mut context = mruby_compiler2_sys::MRubyCompiler2Context::new();
        context.dump_bytecode(code).unwrap();
        context.compile(code).unwrap()
    }
}

pub(crate) fn int(n: i64) -> Value {
    Value::Integer(n)
}

pub(crate) fn string(s: &str) -> Value {
    Value::from_rc(Rc::new(RObject::string(s.to_string())))
}

pub(crate) fn symbol(s: &str) -> Value {
    Value::from_rc(Rc::new(RObject::symbol(RSym::new(s.to_string()))))
}
