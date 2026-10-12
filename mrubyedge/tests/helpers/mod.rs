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

pub(crate) fn mrbc_compile_with_debug(code: &str) -> Vec<u8> {
    // The compiler wrapper uses dump flags = 0. Call its underlying C API
    // with MRC_DUMP_DEBUG_INFO to exercise real compiler-generated DBG data.
    use std::ffi::{c_int, c_void};
    unsafe extern "C" {
        fn mrc_ccontext_new(mrb: *mut c_void) -> *mut c_void;
        fn mrc_ccontext_free(c: *mut c_void);
        fn mrc_load_string_cxt(c: *mut c_void, source: *mut *const u8, len: usize) -> *mut c_void;
        fn mrc_dump_irep(
            c: *mut c_void,
            irep: *const c_void,
            flags: u8,
            bin: *mut *mut u8,
            size: *mut usize,
        ) -> c_int;
        fn mrc_irep_free(c: *mut c_void, irep: *mut c_void);
        fn free(ptr: *mut c_void);
    }

    unsafe {
        let context = mrc_ccontext_new(std::ptr::null_mut());
        assert!(!context.is_null());
        let mut source = code.as_ptr();
        let irep = mrc_load_string_cxt(context, &mut source, code.len());
        if irep.is_null() {
            mrc_ccontext_free(context);
            panic!("Failed to compile Ruby source");
        }
        let mut bin = std::ptr::null_mut();
        let mut size = 0;
        let result = mrc_dump_irep(context, irep, 1, &mut bin, &mut size);
        let binary = if result == 0 && !bin.is_null() {
            std::slice::from_raw_parts(bin, size).to_vec()
        } else {
            Vec::new()
        };
        free(bin.cast());
        mrc_irep_free(context, irep);
        mrc_ccontext_free(context);
        assert_eq!(result, 0, "Failed to dump debug bytecode");
        binary
    }
}
