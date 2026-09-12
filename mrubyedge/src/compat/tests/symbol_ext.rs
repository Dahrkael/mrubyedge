// Tests for the compat Symbol additions.

use super::{as_bool, as_i64, as_str, as_vec, eval_ok, vm};

#[test]
fn case_transformations_round_trip() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[:hello.upcase.to_s, :Hello.downcase.to_s, :hello.capitalize.to_s, :hi.length, :abc.empty?, :a.succ.to_s]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "HELLO");
    assert_eq!(as_str(&v[1]), "hello");
    assert_eq!(as_str(&v[2]), "Hello");
    assert_eq!(as_i64(&v[3]), 2);
    assert!(!as_bool(&v[4]));
    assert_eq!(as_str(&v[5]), "b");
}

#[test]
fn ordering_and_casecmp() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[:abc <=> :abd, :zz <=> :aa, :abc.casecmp(:ABC), :m.between?(:a, :z), :abc[1]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), -1);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 0);
    assert!(as_bool(&v[3]));
    assert_eq!(as_str(&v[4]), "b");
}
