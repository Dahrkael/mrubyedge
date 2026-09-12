// Tests for the compat Range additions.

use super::{as_bool, as_i64, as_str, as_vec, eval_ok, vm};

#[test]
fn triple_equal_powers_case_when() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "classify = lambda do |n|
  case n
  when 1..3 then 'low'
  when 4..6 then 'mid'
  else 'high'
  end
end\n[classify.call(2), classify.call(5), classify.call(9), (1...5).cover?(5), (1..5).cover?(5), (1..5).include?(3)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "low");
    assert_eq!(as_str(&v[1]), "mid");
    assert_eq!(as_str(&v[2]), "high");
    assert!(!as_bool(&v[3]));
    assert!(as_bool(&v[4]));
    assert!(as_bool(&v[5]));
}

#[test]
fn bounds_and_size() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "r = (2..7)\ne = (2...7)\n[r.begin, r.end, e.exclude_end?, r.size, e.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 2);
    assert_eq!(as_i64(&v[1]), 7);
    assert!(as_bool(&v[2]));
    assert_eq!(as_i64(&v[3]), 6);
    assert_eq!(as_i64(&v[4]), 5);
}

#[test]
fn first_last_slices() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "r = (10..20)\nf = r.first(3)\nl = r.last(2)\n[f[0], f[2], l[0], l[1]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 10);
    assert_eq!(as_i64(&v[1]), 12);
    assert_eq!(as_i64(&v[2]), 19);
    assert_eq!(as_i64(&v[3]), 20);
}

#[test]
fn last_honors_exclusive_end_and_lower_bound() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "e = (1...5).last(3)\nc = (5..10).last(100)\n[e.join(','), c.join(','), (1...5).last]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "2,3,4", "exclusive end drops 5");
    assert_eq!(as_str(&v[1]), "5,6,7,8,9,10", "count clamps to the range");
    assert_eq!(as_i64(&v[2]), 5, "no-arg last returns the end value");
}

#[test]
fn step_walks_with_stride() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "out = []\n(0..10).step(3) { |i| out.push(i) }\n[out.join(',')]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "0,3,6,9");
}

#[test]
fn float_bounds_still_work_for_cover() {
    let mut vm = vm();
    let r = eval_ok(&mut vm, "(1.5..2.5).cover?(2.0)");
    assert!(as_bool(&r));
}

#[test]
fn step_near_i64_max_does_not_overflow() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "out = []
a = Integer('9223372036854775807')
(a..a).step(2) { |i| out.push(i) }
out.size",
    );
    assert_eq!(as_i64(&r), 1);
}
