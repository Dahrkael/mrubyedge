#![allow(unused_imports)]
#![allow(dead_code)]
use std::rc::Rc;

use mrubyedge::yamrb::value::RObject;

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

pub(crate) fn int(n: i64) -> Rc<RObject> {
    Rc::new(RObject::integer(n))
}

pub(crate) fn string(s: &str) -> Rc<RObject> {
    Rc::new(RObject::string(s.to_string()))
}
