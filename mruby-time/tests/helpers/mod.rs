#![allow(dead_code)]

pub fn mrbc_compile(_fname: &'static str, code: &str) -> Vec<u8> {
    unsafe {
        let mut context = mruby_compiler2_sys::MRubyCompiler2Context::new();
        context.compile(code).unwrap()
    }
}

pub fn mrbc_compile_debug(_fname: &'static str, code: &str) -> Vec<u8> {
    unsafe {
        let mut context = mruby_compiler2_sys::MRubyCompiler2Context::new();
        context.dump_bytecode(code).unwrap();
        context.compile(code).unwrap()
    }
}
