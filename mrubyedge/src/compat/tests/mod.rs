// Shared test helpers for the compat layer: build a registered VM, evaluate
// snippets, extract values.

use crate::yamrb::value::Value;
use crate::yamrb::vm::VM;

mod array;
mod comparable;
mod control_flow;
mod enumerable;
mod exceptions;
mod hash;
mod math;
mod numeric;
mod object_ext;
mod proc_ext_test;
mod range_extras;
mod string;
mod symbol_ext;

pub(crate) fn vm() -> VM {
    let mut vm = VM::empty();
    super::register(&mut vm).expect("compat register");
    vm
}

pub(crate) fn eval_ok(vm: &mut VM, src: &str) -> Value {
    let blob = unsafe {
        let mut ctx = mruby_compiler2_sys::MRubyCompiler2Context::new();
        ctx.compile(src).expect("compile")
    };
    let mut rite = crate::rite::load(&blob).expect("rite");
    vm.eval_rite(&mut rite)
        .unwrap_or_else(|e| panic!("eval failed: {e}: {src}"))
}

/// Evaluates a snippet expected to raise; returns the rendered error text.
pub(crate) fn eval_err(vm: &mut VM, src: &str) -> String {
    let blob = unsafe {
        let mut ctx = mruby_compiler2_sys::MRubyCompiler2Context::new();
        ctx.compile(src).expect("compile")
    };
    let mut rite = crate::rite::load(&blob).expect("rite");
    let err = vm
        .eval_rite(&mut rite)
        .expect_err("snippet was expected to raise");
    describe_error(err.as_ref())
}

/// Compact "Type: message" rendering of VM errors.
fn describe_error(err: &(dyn std::error::Error + 'static)) -> String {
    let Some(e) = err.downcast_ref::<crate::Error>() else {
        return err.to_string();
    };
    let named = |name: &str, msg: &String| -> String {
        if msg.is_empty() {
            name.to_string()
        } else {
            format!("{name}: {msg}")
        }
    };
    match e {
        crate::Error::RuntimeError(m) => named("RuntimeError", m),
        crate::Error::ArgumentError(m) => named("ArgumentError", m),
        crate::Error::NoMethodError(m) => named("NoMethodError", m),
        crate::Error::NameError(m) => named("NameError", m),
        crate::Error::RangeError(m) => named("RangeError", m),
        crate::Error::Internal(m) => named("InternalError", m),
        crate::Error::TaggedError(tag, m) => named(tag, m),
        crate::Error::LocalJumpError(m) => named("LocalJumpError", m),
        crate::Error::TypeMismatch => "TypeMismatch".to_string(),
        crate::Error::ZeroDivisionError => "ZeroDivisionError".to_string(),
        crate::Error::InvalidOpCode => "InvalidOpCode".to_string(),
        crate::Error::General => "ScriptError".to_string(),
        _ => "ScriptError".to_string(),
    }
}

pub(crate) fn as_i64(v: &Value) -> i64 {
    i64::try_from(v).expect("Integer")
}

pub(crate) fn as_f64(v: &Value) -> f64 {
    f64::try_from(v).expect("number")
}

pub(crate) fn as_str(v: &Value) -> String {
    String::try_from(v).expect("String")
}

pub(crate) fn as_bool(v: &Value) -> bool {
    bool::try_from(v).expect("Bool")
}

pub(crate) fn as_vec(v: &Value) -> Vec<Value> {
    Vec::try_from(v).expect("Array")
}

pub(crate) fn feq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}
mod module_interplay;
