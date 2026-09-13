#![allow(dead_code)]

pub fn mrbc_compile(_fname: &'static str, code: &'static str) -> Vec<u8> {
    unsafe {
        let mut context = mruby_compiler2_sys::MRubyCompiler2Context::new();
        context.compile(code).unwrap()
    }
}
