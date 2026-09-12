// Tests for the Math module extension.

use super::{as_f64, eval_err, eval_ok, feq, vm};

#[test]
fn trig_values() {
    let mut vm = vm();
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.sin(0)")), 0.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.cos(0)")), 1.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.tan(0)")), 0.0);
    assert!(feq(
        as_f64(&eval_ok(&mut vm, "Math.sin(Math::PI / 2)")),
        1.0
    ));
}

#[test]
fn inverse_trig_and_quadrants() {
    let mut vm = vm();
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.asin(0)")), 0.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.acos(1)")), 0.0);
    assert!(feq(
        as_f64(&eval_ok(&mut vm, "Math.atan(1)")),
        std::f64::consts::FRAC_PI_4
    ));
    // atan2 covers all four quadrants.
    assert!(feq(
        as_f64(&eval_ok(&mut vm, "Math.atan2(1, 1)")),
        std::f64::consts::FRAC_PI_4
    ));
    assert!(feq(
        as_f64(&eval_ok(&mut vm, "Math.atan2(-1, -1)")),
        -3.0 * std::f64::consts::FRAC_PI_4
    ));
    assert!(feq(
        as_f64(&eval_ok(&mut vm, "Math.atan2(0, -1)")),
        std::f64::consts::PI
    ));
}

#[test]
fn hyperbolic_values() {
    let mut vm = vm();
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.sinh(0)")), 0.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.cosh(0)")), 1.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.tanh(0)")), 0.0);
}

#[test]
fn logs_exp_powers() {
    let mut vm = vm();
    assert!(feq(as_f64(&eval_ok(&mut vm, "Math.log(Math::E)")), 1.0));
    assert!(feq(as_f64(&eval_ok(&mut vm, "Math.log(8, 2)")), 3.0));
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.log2(8)")), 3.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.log10(100)")), 2.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.exp(0)")), 1.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.pow(2, 10)")), 1024.0);
    assert!(feq(as_f64(&eval_ok(&mut vm, "Math.cbrt(27)")), 3.0));
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.hypot(3, 4)")), 5.0);
}

#[test]
fn constants() {
    let mut vm = vm();
    assert!(feq(
        as_f64(&eval_ok(&mut vm, "Math::PI")),
        std::f64::consts::PI
    ));
    assert!(feq(
        as_f64(&eval_ok(&mut vm, "Math::E")),
        std::f64::consts::E
    ));
}

#[test]
fn log_of_zero_is_negative_infinity() {
    // Ruby returns -Infinity for log(0); only negatives raise.
    let log = as_f64(&eval_ok(&mut vm(), "Math.log(0)"));
    assert!(log.is_infinite() && log < 0.0, "{log}");
    assert_eq!(
        as_f64(&eval_ok(&mut vm(), "Math.log2(0)")),
        f64::NEG_INFINITY
    );
    assert_eq!(
        as_f64(&eval_ok(&mut vm(), "Math.log10(0.0)")),
        f64::NEG_INFINITY
    );
}

#[test]
fn domain_errors_raise_range_error() {
    let cases = [
        "Math.sqrt(-1)",
        "Math.log(-4, 2)",
        "Math.log2(-8)",
        "Math.log10(-100)",
        "Math.asin(2)",
        "Math.acos(-1.5)",
        "Math.log(8, 1)",
    ];
    for src in cases {
        let msg = eval_err(&mut vm(), src);
        assert!(msg.contains("RangeError"), "{src} => {msg}");
    }
}

#[test]
fn legacy_entries_still_work() {
    let mut vm = vm();
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.sqrt(4)")), 2.0);
    assert_eq!(as_f64(&eval_ok(&mut vm, "Math.abs(-3.5)")), 3.5);
}
