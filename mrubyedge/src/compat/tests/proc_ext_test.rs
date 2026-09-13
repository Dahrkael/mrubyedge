// Tests for the compat Proc additions.

use super::{as_i64, as_str, as_vec, eval_ok, vm};

#[test]
fn proc_triple_equal_calls() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "doubler = ->(x) { x * 2 }\n[doubler === 21, doubler.call(5)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 42);
    assert_eq!(as_i64(&v[1]), 10);
}

#[test]
fn to_proc_and_arity() {
    let mut vm = vm();
    let r = eval_ok(&mut vm, "p = :upcase.to_proc\n[p.call('ab'), p.arity]");
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "AB");
    assert_eq!(as_i64(&v[1]), -1);
}
