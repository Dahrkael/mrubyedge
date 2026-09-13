// Tests for the Comparable module.

use super::{as_bool, as_i64, as_vec, eval_ok, vm};

#[test]
fn user_class_derives_operators_from_spaceship() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "class Level
  include Comparable
  def initialize(v)
    @v = v
  end
  def value
    @v
  end
  def <=>(other)
    @v <=> other.value
  end
end\na = Level.new(1)\nb = Level.new(2)\n[a < b, a <= a, b > a, a >= b, a == Level.new(1)]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]));
    assert!(as_bool(&v[1]));
    assert!(as_bool(&v[2]));
    assert!(!as_bool(&v[3]));
    assert!(as_bool(&v[4]));
}

#[test]
fn between_and_clamp() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[5.between?(1, 10), 0.between?(1, 10), 7.clamp(1, 5), -3.clamp(1, 5), 'm'.between?('a', 'z')]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]));
    assert!(!as_bool(&v[1]));
    assert_eq!(as_i64(&v[2]), 5);
    assert_eq!(as_i64(&v[3]), 1);
    assert!(as_bool(&v[4]));
}

#[test]
fn string_ordering_operators() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "['apple' < 'banana', 'b' >= 'c', 'same' == 'SAME'.downcase]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]));
    assert!(!as_bool(&v[1]));
    assert!(as_bool(&v[2]));
}
