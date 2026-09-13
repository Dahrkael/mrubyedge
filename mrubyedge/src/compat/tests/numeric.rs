// Tests for the compat Integer/Float additions.

use super::{as_bool, as_f64, as_i64, as_str, as_vec, eval_ok, vm};

#[test]
fn upto_downto_and_step_walks() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "u = []\n1.upto(4) { |i| u.push(i) }\nd = []\n3.downto(1) { |i| d.push(i) }\ns = []\n1.step(7, 3) { |i| s.push(i) }\nb = []\n10.step(4, -2) { |i| b.push(i) }\n[u.join(','), d.join(','), s.join(','), b.join(',')]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "1,2,3,4");
    assert_eq!(as_str(&v[1]), "3,2,1");
    assert_eq!(as_str(&v[2]), "1,4,7");
    assert_eq!(as_str(&v[3]), "10,8,6,4");
}

#[test]
fn parity_sign_succ_pred() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[4.even?, 4.odd?, 0.zero?, 5.positive?, -5.negative?, 5.succ, 5.pred]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]));
    assert!(!as_bool(&v[1]));
    assert!(as_bool(&v[2]));
    assert!(as_bool(&v[3]));
    assert!(as_bool(&v[4]));
    assert_eq!(as_i64(&v[5]), 6);
    assert_eq!(as_i64(&v[6]), 4);
}

#[test]
fn division_family() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "dm = -7.divmod(2)\n[-7.fdiv(2), dm[0], dm[1], -7.div(2), 2.pow(10)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_f64(&v[0]), -3.5);
    assert_eq!(as_i64(&v[1]), -4);
    assert_eq!(as_i64(&v[2]), 1);
    assert_eq!(as_i64(&v[3]), -4);
    assert_eq!(as_i64(&v[4]), 1024);
}

#[test]
fn digits_gcd_lcm_bit_length() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "d = 255.digits(16)\n[d[0], d[1], 12.gcd(18), 4.lcm(6), 8.bit_length]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 15);
    assert_eq!(as_i64(&v[1]), 15);
    assert_eq!(as_i64(&v[2]), 6);
    assert_eq!(as_i64(&v[3]), 12);
    assert_eq!(as_i64(&v[4]), 4);
}

#[test]
#[allow(clippy::approx_constant)]
fn float_rounding_family() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[3.7.round, 3.2.round, -3.7.round, 3.9.floor, 3.1.ceil, -3.9.truncate, (3.14159).round(2), (2.5).round, (-2.5).round]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 4);
    assert_eq!(as_i64(&v[1]), 3);
    assert_eq!(as_i64(&v[2]), -4);
    assert_eq!(as_f64(&v[3]), 3.0);
    assert_eq!(as_f64(&v[4]), 4.0);
    assert_eq!(as_i64(&v[5]), -3);
    assert_eq!(as_f64(&v[6]), 3.14);
    assert_eq!(as_i64(&v[7]), 3);
    assert_eq!(as_i64(&v[8]), -3);
}

#[test]
fn coerce_pairs_floats() {
    let mut vm = vm();
    let r = eval_ok(&mut vm, "c = 3.coerce(2.5)\n[c[0], c[1]]");
    let v = as_vec(&r);
    assert_eq!(as_f64(&v[0]), 2.5);
    assert_eq!(as_f64(&v[1]), 3.0);
}

#[test]
fn float_modulo_matches_ruby_semantics() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[5.0 % 3.0, -5.0 % 3.0, 5.0 % -3.0, (7.5).modulo(2.0), 5.0.divmod(3.0)[0], 5.0.divmod(3.0)[1], (-5.5).remainder(2.0)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_f64(&v[0]), 2.0);
    assert_eq!(as_f64(&v[1]), 1.0);
    assert_eq!(as_f64(&v[2]), -1.0);
    assert_eq!(as_f64(&v[3]), 1.5);
    assert_eq!(as_f64(&v[4]), 1.0);
    assert_eq!(as_f64(&v[5]), 2.0);
    assert_eq!(as_f64(&v[6]), -1.5);
}

#[test]
fn integer_modulo_is_floored() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[-7 % 2, 7 % -3, (-7).modulo(2), 7.modulo(-3), 7 % 2.5, 4.5 % 2]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1); // -7 / 2 floors to -4, remainder 1
    assert_eq!(as_i64(&v[1]), -2); // 7 / -3 floors to -3, remainder -2
    assert_eq!(as_i64(&v[2]), 1);
    assert_eq!(as_i64(&v[3]), -2);
    assert_eq!(as_f64(&v[4]), 2.0);
    assert_eq!(as_f64(&v[5]), 0.5);
}

#[test]
fn modulo_by_zero_raises() {
    let mut vm = vm();
    let msg = super::eval_err(&mut vm, "7 % 0");
    assert!(msg.contains("ZeroDivisionError"), "{msg}");
    let msg = super::eval_err(&mut vm, "7.0 % 0.0");
    assert!(msg.contains("ZeroDivisionError"), "{msg}");
}

#[test]
fn modulo_assignment_wraps_positions() {
    let mut vm = vm();
    let r = eval_ok(&mut vm, "x = 17.0\nancho = 10.0\nx %= ancho\n[x, 17 % 10]");
    let v = as_vec(&r);
    assert_eq!(as_f64(&v[0]), 7.0);
    assert_eq!(as_i64(&v[1]), 7);
}

#[test]
fn float_round_negative_digits() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[15.5.round(-1), (-15.5).round(-1), 1234.5.round(-2), 1234.5.floor(-2), 1234.5.ceil(-2), 1234.5.truncate(-2)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 20);
    assert_eq!(as_i64(&v[1]), -20);
    assert_eq!(as_i64(&v[2]), 1200);
    assert_eq!(as_i64(&v[3]), 1200);
    assert_eq!(as_i64(&v[4]), 1300);
    assert_eq!(as_i64(&v[5]), 1200);
}

#[test]
fn integer_mod_and_gcd_handle_i64_min() {
    // Kernel#Integer(String) reaches the true i64 bounds the compiler's own
    // literals cannot represent.
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "m = Integer('-9223372036854775808')\n[m.modulo(-1), m.gcd(1), m.even?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 0, "i64::MIN % -1 must not overflow");
    assert_eq!(as_i64(&v[1]), 1, "gcd must not panic on i64::MIN");
    assert!(as_bool(&v[2]));
}

#[test]
fn integer_overflow_raises_instead_of_panicking() {
    let msg = super::eval_err(&mut vm(), "Integer('-9223372036854775808').pred");
    assert!(msg.contains("RangeError"), "{msg}");
    let msg = super::eval_err(&mut vm(), "Integer('9223372036854775807').succ");
    assert!(msg.contains("RangeError"), "{msg}");
    let msg = super::eval_err(&mut vm(), "Integer('-9223372036854775808').divmod(-1)");
    assert!(msg.contains("RangeError"), "{msg}");
}

#[test]
fn modulo_without_argument_raises() {
    let msg = super::eval_err(&mut vm(), "5.modulo");
    assert!(msg.contains("ArgumentError"), "{msg}");
}

#[test]
fn integer_divmod_does_not_overflow() {
    // These quotients need an i128 intermediate: (a - r) overflows i64.
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = Integer('-9223372036854775808')
b = Integer('9223372036854775807')
d1 = a.divmod(b)
d2 = 1.divmod(a)
[d1[0], d1[1], d2[0], d2[1]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), -2);
    assert_eq!(as_i64(&v[1]), 9223372036854775806);
    assert_eq!(as_i64(&v[2]), -1);
    assert_eq!(as_i64(&v[3]), -9223372036854775807);
}
