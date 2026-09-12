// Tests for the compat Exception additions.

use super::{as_bool, as_str, as_vec, eval_err, eval_ok, vm};
use crate::yamrb::vm::VM;

#[test]
fn new_exception_classes_exist_and_rescue() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[KeyError < StandardError, IndexError < StandardError, StopIteration < StandardError,
 FrozenError < StandardError, LocalJumpError < StandardError, IOError < StandardError]",
    );
    let v = as_vec(&r);
    assert_eq!(v.len(), 6);
    for (i, x) in v.iter().enumerate() {
        assert!(as_bool(x), "class {i}");
    }
}

#[test]
fn exception_new_stores_message() {
    let mut vm = vm();
    let r = eval_ok(&mut vm, "e = RuntimeError.new('boom')\n[e.message, e.to_s]");
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "boom");
    assert_eq!(as_str(&v[1]), "boom");
    let insp = eval_ok(&mut vm, "StandardError.new('oops').inspect");
    assert_eq!(as_str(&insp), "#<StandardError: oops>");
}

#[test]
fn raise_with_custom_instance_carries_message() {
    let mut vm = vm();
    let err = eval_err(
        &mut vm,
        "class AppError < StandardError; end\nraise AppError.new('bad input')",
    );
    assert!(err.contains("bad input"), "{err}");
}

/// Upstream mrubyedge overflows the stack instantiating exception classes
/// through the generic Class#new. The native Exception#new bypasses it; this
/// frozen reproducer documents the underlying defect. Run manually with
/// `cargo test -p mrubyedge -- --ignored` (it aborts the process).
#[test]
#[ignore = "upstream stack overflow in generic Class#new for exceptions"]
fn repro_upstream_exception_new_overflow() {
    let mut v = VM::empty();
    let blob = unsafe {
        let mut ctx = mruby_compiler2_sys::MRubyCompiler2Context::new();
        ctx.compile("RuntimeError.new('x')").expect("compile")
    };
    let mut rite = crate::rite::load(&blob).expect("rite");
    // On a fixed VM this evaluates (or raises cleanly) instead of aborting.
    let _ = v.eval_rite(&mut rite);
}
