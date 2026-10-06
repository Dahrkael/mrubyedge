extern crate mrubyedge;

mod helpers;
use helpers::*;

use num_bigint::BigInt;
use num_traits::{Num, ToPrimitive};
use std::rc::Rc;

use mrubyedge::yamrb::value::Value;

fn run(code: &'static str) -> Value {
    let binary = mrbc_compile("bigint", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap()
}

fn big(n: BigInt) -> Rc<BigInt> {
    Rc::new(n)
}

#[test]
fn bigint_decimal_literal_roundtrip() {
    let result = run(r#"
x = 99999999999999999999999
x.to_s
"#);
    let expected = BigInt::from_str_radix("99999999999999999999999", 10).unwrap();
    let s: String = result.try_into().expect("bigint to_s string");
    assert_eq!(s, expected.to_string());
}

#[test]
fn bigint_hex_literal_roundtrip() {
    let result = run(r#"
x = 0x1fffffffffffffff0000000000
x.to_s
"#);
    let expected = BigInt::from_str_radix("1fffffffffffffff0000000000", 16).unwrap();
    let s: String = result.try_into().expect("bigint hex to_s string");
    assert_eq!(s, expected.to_string());
}

#[test]
fn bigint_negative_literal_roundtrip() {
    let result = run(r#"
x = -99999999999999999999999
x.to_s
"#);
    let s: String = result.try_into().expect("negative bigint to_s string");
    assert_eq!(s, "-99999999999999999999999");
}

#[test]
fn bigint_arithmetic_add_sub_mul_div() {
    let a = BigInt::from_str_radix("99999999999999999999999", 10).unwrap();
    let b = BigInt::from_str_radix("12345678901234567890", 10).unwrap();

    let result = run(r#"
a = 99999999999999999999999
b = 12345678901234567890
[a + b, a - b, a * b, a / b]
"#);
    let arr: Vec<Value> = result.try_into().expect("arithmetic array");

    let expected = [
        &a + &b,
        &a - &b,
        &a * &b,
        // num-bigint `/` truncates toward zero, like Ruby Integer division.
        &a / &b,
    ];
    for (got, exp) in arr.iter().zip(expected.iter()) {
        match got {
            Value::BigInt(n) => assert_eq!(n.as_ref(), exp),
            other => panic!("expected bigint, got {:?}", other),
        }
    }
}

#[test]
fn bigint_division_truncates_toward_zero() {
    let result = run(r#"
a = -99999999999999999999999
b = 12345678901234567890
q = a / b
[a / b, q * b >= a]
"#);
    let arr: Vec<Value> = result.try_into().expect("truncation array");
    let a = -BigInt::from_str_radix("99999999999999999999999", 10).unwrap();
    let b = BigInt::from_str_radix("12345678901234567890", 10).unwrap();
    match &arr[0] {
        Value::BigInt(n) => {
            assert_eq!(n.as_ref(), &(&a / &b));
            // Truncation: quotient times divisor is closer to zero than a.
            assert!(n.as_ref() * &b >= a);
        }
        other => panic!("expected bigint quotient, got {:?}", other),
    }
    let within: bool = arr[1].clone().try_into().expect("bool");
    assert!(within);
}

#[test]
fn bigint_promotion_from_integer_addition() {
    let result = run(r#"
1 + 99999999999999999999999
"#);
    let expected = BigInt::from(1) + BigInt::from_str_radix("99999999999999999999999", 10).unwrap();
    match result {
        Value::BigInt(n) => assert_eq!(n.as_ref(), &expected),
        other => panic!("expected bigint, got {:?}", other),
    }
}

#[test]
fn bigint_subtraction_to_zero_compares_with_integer() {
    let result = run(r#"
a = 99999999999999999999999
z = a - a
[z == 0, z.to_s]
"#);
    let arr: Vec<Value> = result.try_into().expect("zero array");
    let eq_small: bool = arr[0].clone().try_into().expect("bool");
    assert!(eq_small);
    let s: String = arr[1].clone().try_into().expect("zero to_s");
    assert_eq!(s, "0");
}

#[test]
fn bigint_division_by_zero_is_error() {
    let binary = mrbc_compile("bigint_div0", "x = 99999999999999999999999; x / 0");
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let err = vm.run().unwrap_err();
    let err = err.downcast_ref::<mrubyedge::Error>().expect("a VM error");
    assert!(
        matches!(err, mrubyedge::Error::ZeroDivisionError),
        "expected ZeroDivisionError, got {:?}",
        err
    );
}

#[test]
fn bigint_comparisons() {
    let result = run(r#"
a = 99999999999999999999999
b = 99999999999999999999998
[a < b, b < a, a == a, a == 1, a > 1, a < 9.5, a > 9.5, 1 < a]
"#);
    let arr: Vec<Value> = result.try_into().expect("comparison array");
    let expected = [false, true, true, false, true, false, true, true];
    for (got, exp) in arr.iter().zip(expected.iter()) {
        let b: bool = got.clone().try_into().expect("comparison bool");
        assert_eq!(b, *exp);
    }
}

#[test]
fn bigint_truthiness() {
    // The VM has no `!` operator (pre-existing gap); ternary dispatches
    // through the same Value::is_truthy path. In Ruby every integer is
    // truthy, including zero — bigints included.
    let result = run(r#"
a = 99999999999999999999999
z = a - a
[a ? 1 : 0, z ? 1 : 0]
"#);
    let arr: Vec<Value> = result.try_into().expect("truthiness array");
    let non_zero: i64 = arr[0].clone().try_into().expect("int");
    let zero: i64 = arr[1].clone().try_into().expect("int");
    assert_eq!(non_zero, 1);
    assert_eq!(zero, 1);
}

#[test]
fn bigint_string_interpolation() {
    let result = run(r#"
a = 99999999999999999999999
"v:#{a}"
"#);
    let expected = BigInt::from_str_radix("99999999999999999999999", 10).unwrap();
    let s: String = result.try_into().expect("interpolated string");
    assert_eq!(s, format!("v:{}", expected));
}

#[test]
fn bigint_as_hash_key() {
    let result = run(r#"
a = 99999999999999999999999
h = { a => 42 }
h[a]
"#);
    let v: i64 = result.try_into().expect("hash lookup integer");
    assert_eq!(v, 42);
}

#[test]
fn bigint_abs() {
    let result = run(r#"
a = -99999999999999999999999
a.abs.to_s
"#);
    let expected = BigInt::from_str_radix("99999999999999999999999", 10).unwrap();
    let s: String = result.try_into().expect("abs to_s string");
    assert_eq!(s, expected.to_string());
}

#[test]
fn bigint_value_semantics_in_rust() {
    // Sanity-check the Rust-side plumbing: boxing, unboxing, hashing and the
    // Integer/BigInt equality bridge used by Hash and ==.
    let n = big(BigInt::from_str_radix("99999999999999999999999", 10).unwrap());
    let v = Value::BigInt(n);
    let rc = v.to_rc();
    assert!(matches!(rc.tt, mrubyedge::yamrb::value::RType::BigInt));
    assert!(matches!(Value::from_rc(rc), Value::BigInt(_)));

    let small = Value::BigInt(big(BigInt::from(7)));
    assert_eq!(small, Value::Integer(7));
    assert_eq!(
        small.as_hash_key().unwrap(),
        Value::Integer(7).as_hash_key().unwrap()
    );

    let zero = Value::BigInt(big(BigInt::from(0)));
    // Every integer is truthy in Ruby, zero included.
    assert!(zero.is_truthy());
    assert!(v.is_truthy());

    let s: String = (&v).try_into().unwrap();
    assert_eq!(s, "99999999999999999999999");
    match &v {
        Value::BigInt(b) => assert_eq!(b.to_i64(), None),
        _ => panic!("expected bigint"),
    }
}
