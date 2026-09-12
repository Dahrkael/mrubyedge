//! Regression tests for the call-frame / callinfo machinery. The frame stack
//! rewrite pools callinfos and must preserve exactly these behaviours:
//! deep-recursion recovery, native<->Ruby funcall nesting, `super` resolution
//! through the frame, exception unwinding, block upvars and aliases.

extern crate mec_mrbc_sys;
extern crate mrubyedge;

mod helpers;

use helpers::*;
use mrubyedge::Error;
use mrubyedge::yamrb::helpers::mrb_define_cmethod;
use mrubyedge::yamrb::value::Value;
use mrubyedge::yamrb::vm::VM;

fn run_source(name: &'static str, code: &'static str) -> VM {
    let binary = mrbc_compile(name, code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    vm
}

fn call_i64(vm: &mut VM, name: &str, args: &[Value]) -> i64 {
    mrb_funcall(vm, None, name, args)
        .unwrap_or_else(|e| panic!("{name} raised {e:?}"))
        .try_into()
        .unwrap()
}

/// Unbounded recursion must raise a Ruby SystemStackError (not a Rust stack
/// overflow / panic) and leave the VM usable: the outermost frame restores the
/// register offset and callinfo chain on the way out.
#[test]
fn deep_recursion_raises_and_recovers() {
    let code = "
def recurse(n)
  1 + recurse(n + 1)
end
def ok
  4242
end
";
    let mut vm = run_source("frame_deep_recursion", code);

    let err = mrb_funcall(&mut vm, None, "recurse", &[Value::Integer(0)])
        .expect_err("unbounded recursion must raise");
    assert!(
        matches!(&err, Error::TaggedError(class, _) if class == "SystemStackError"),
        "expected SystemStackError, got {err:?}"
    );

    // Frame bookkeeping must be balanced after the unwind: the register
    // offset, the callinfo chain and the breadcrumb stack all back to empty.
    assert_eq!(vm.current_regs_offset, 0);
    assert!(vm.current_callinfo.is_none());
    assert_eq!(vm.breadcrumbs.borrow().len(), 0);

    // Consume the pending exception (the host's job) and the VM is usable.
    vm.exception.take();
    assert_eq!(call_i64(&mut vm, "ok", &[]), 4242);
    // A second overflow must fail the same way (no leaked/duplicated frames).
    let err2 = mrb_funcall(&mut vm, None, "recurse", &[Value::Integer(0)]).expect_err("again");
    assert!(matches!(&err2, Error::TaggedError(class, _) if class == "SystemStackError"));
    assert_eq!(vm.current_regs_offset, 0);
    assert!(vm.current_callinfo.is_none());
    vm.exception.take();
    assert_eq!(call_i64(&mut vm, "ok", &[]), 4242);
}

/// Recursion that stays within the register window must produce the exact
/// value: frame push/pop must not corrupt registers near the limit.
#[test]
fn deep_but_valid_recursion_returns_value() {
    let code = "
def countdown(n)
  return 0 if n == 0
  1 + countdown(n - 1)
end
";
    let mut vm = run_source("frame_valid_recursion", code);
    assert_eq!(call_i64(&mut vm, "countdown", &[Value::Integer(40)]), 40);
    assert_eq!(call_i64(&mut vm, "countdown", &[Value::Integer(0)]), 0);
}

/// A native method that calls back into Ruby via `mrb_funcall`, invoked from
/// Ruby, invoked again by Rust. Exercises the funcall callinfo save/restore
/// (`is_funcall` return path) that the pooled stack must reproduce.
#[test]
fn native_funcall_round_trip() {
    let code = "
def inc(n)
  n + 1
end
def through_native(n)
  rust_inc_twice(n)
end
";
    let mut vm = run_source("frame_native_round_trip", code);

    fn rust_inc_twice(vm: &mut VM, args: &[Option<Value>]) -> Result<Value, Error> {
        let n: i64 = args[0].as_ref().unwrap().try_into()?;
        let a = mrb_funcall(vm, None, "inc", &[Value::Integer(n)])?;
        let b = mrb_funcall(vm, None, "inc", &[Value::Integer(n + 10)])?;
        let a: i64 = a.try_into()?;
        let b: i64 = b.try_into()?;
        Ok(Value::Integer(a + b))
    }

    let kernel = vm.object_class.clone();
    mrb_define_cmethod(&mut vm, kernel, "rust_inc_twice", Box::new(rust_inc_twice));

    assert_eq!(
        call_i64(&mut vm, "through_native", &[Value::Integer(5)]),
        5 + 1 + 15 + 1
    );
}

/// `super` resolves through the live frame: the callinfo must still expose the
/// right method id/owner when the call is a few frames deep.
#[test]
fn super_through_nested_frames() {
    let code = "
class A
  def who; 1; end
end
class B < A
  def who; super + 1; end
end
class C < B
  def who; super + 1; end
end
def helper; C.new.who; end
def run; helper + helper; end
";
    let mut vm = run_source("frame_super_nested", code);
    assert_eq!(call_i64(&mut vm, "run", &[]), 6);
}

/// An exception raised several frames deep, rescued, must unwind cleanly and
/// leave the VM callable again.
#[test]
fn exception_unwinds_frames_and_recovers() {
    let code = "
def a; b; end
def b; c; end
def c; raise 'boom'; end
def safe
  a
  0
rescue => e
  7
end
def ok; 9; end
";
    let mut vm = run_source("frame_exception_unwind", code);
    assert_eq!(call_i64(&mut vm, "safe", &[]), 7);
    assert_eq!(call_i64(&mut vm, "ok", &[]), 9);
}

/// Blocks capture the enclosing frame's locals through the environment; the
/// per-frame register window must be tracked so upvars survive nested calls.
#[test]
fn block_upvars_across_nested_calls() {
    let code = "
def make
  x = 0
  3.times { x += 2 }
  helper(x)
end
def helper(v)
  [1, 2, 3].each { v += 1 }
  v
end
def run; make; end
";
    let mut vm = run_source("frame_block_upvars", code);
    assert_eq!(call_i64(&mut vm, "run", &[]), 6 + 3);
}

/// `return` from a block unwinds to the defining method frame (BlockReturn),
/// even when the yielding method is reached through another call.
#[test]
fn block_return_unwinds_to_defining_frame() {
    let code = "
def inner
  yield
  :after
end
def bridge
  inner { return 11 }
  :unreachable
end
def run; bridge; end
";
    let mut vm = run_source("frame_block_return", code);
    assert_eq!(call_i64(&mut vm, "run", &[]), 11);
}

/// Aliases and redefinition bind distinct procs; the frame must carry the id
/// of the method actually invoked, so `super`/backtraces and re-dispatch are
/// correct after a method is replaced.
#[test]
fn alias_and_redefinition_resolve_distinct_methods() {
    let code = "
class K
  def orig; 1; end
  alias copy orig
  def orig; 2; end
  def both
    [copy, orig]
  end
end
def run
  a, b = K.new.both
  a * 10 + b
end
";
    let mut vm = run_source("frame_alias_redefine", code);
    assert_eq!(call_i64(&mut vm, "run", &[]), 12);
}
