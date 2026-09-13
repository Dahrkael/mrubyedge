// Tests for the compat Object/Kernel additions.

use super::{as_bool, as_f64, as_i64, as_str, as_vec, eval_err, eval_ok, vm};

#[test]
fn immediates_share_flyweight_instances() {
    // F3B: nil/true/false and small integers are shared Rc<RObject>; object
    // identity must follow the MRI id scheme and stay stable.
    use crate::yamrb::value::RObject;
    assert!(std::rc::Rc::ptr_eq(&RObject::nil_rc(), &RObject::nil_rc()));
    assert!(std::rc::Rc::ptr_eq(
        &RObject::boolean_rc(true),
        &RObject::boolean_rc(true)
    ));
    assert!(std::rc::Rc::ptr_eq(
        &RObject::integer_rc(7),
        &RObject::integer_rc(7)
    ));
    assert!(!std::rc::Rc::ptr_eq(
        &RObject::integer_rc(7),
        &RObject::integer_rc(8)
    ));
    // Out-of-range values allocate fresh instances (not shared).
    assert!(!std::rc::Rc::ptr_eq(
        &RObject::integer_rc(256),
        &RObject::integer_rc(256)
    ));
    assert!(!std::rc::Rc::ptr_eq(
        &RObject::integer_rc(-1),
        &RObject::integer_rc(-1)
    ));
    assert_eq!(RObject::nil_rc().object_id.get(), 4);
    assert_eq!(RObject::boolean_rc(true).object_id.get(), 20);
    assert_eq!(RObject::boolean_rc(false).object_id.get(), 0);
    assert_eq!(RObject::integer_rc(5).object_id.get(), 11);

    // Ruby-facing object_id stays stable across calls on shared instances.
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[nil.object_id == nil.object_id, 5.object_id == 5.object_id]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]) && as_bool(&v[1]), "object_id is stable");
}

#[test]
fn instance_variable_reflection() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "class P
  def initialize
    @hp = 30
    @mp = 10
  end
  def bump
    @hp = instance_variable_get(:@hp) + 1
    @luck = 7
    self
  end
end
p = P.new
names = p.instance_variables.map { |s| s.to_s }.sort
[p.instance_variable_get('@hp'), p.instance_variable_defined?(:@mp), names.join(',')]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 30);
    assert!(as_bool(&v[1]));
    assert_eq!(as_str(&v[2]), "@hp,@mp");
}

#[test]
fn ivar_set_updates_object_state() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "o = Object.new
o.instance_variable_set(:@score, 99)
[o.instance_variable_get('@score'), o.instance_variable_defined?(:@nope)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 99);
    assert!(!as_bool(&v[1]));
}

#[test]
fn ivar_reflection_accepts_bare_names() {
    // A name without the @ prefix must address the same ivar as "@name".
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "o = Object.new
o.instance_variable_set(:score, 5)
[o.instance_variable_get(:score), o.instance_variable_get('score'), o.instance_variable_defined?(:score)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 5);
    assert_eq!(as_i64(&v[1]), 5);
    assert!(as_bool(&v[2]));
}

#[test]
fn immediates_reject_ivars() {
    // Immediates are shared flyweights; setting an ivar must be rejected
    // (MRI FrozenError) instead of mutating the shared instance. A fresh VM
    // per raise: the VM keeps the exception across evals once one is set.
    let msg = eval_err(&mut vm(), "5.instance_variable_set(:@x, 1)");
    assert!(
        msg.contains("FrozenError") && msg.contains("Integer"),
        "{msg}"
    );
    let msg = eval_err(&mut vm(), "true.instance_variable_set(:@x, 1)");
    assert!(
        msg.contains("FrozenError") && msg.contains("TrueClass"),
        "{msg}"
    );
    let r = eval_ok(
        &mut vm(),
        "class << 5
  def leaky; end
end
[5.instance_variable_get(:@x).nil?, true.instance_variable_get(:@x).nil?, 5.respond_to?(:leaky), 7.respond_to?(:leaky)]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]) && as_bool(&v[1]), "immediates keep no ivar");
    assert!(
        !as_bool(&v[2]) && !as_bool(&v[3]),
        "sclass defs are not shared"
    );
}

#[test]
fn tap_then_and_send() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "side = []
out = [1, 2].tap { |a| side.push(a) }
mapped = [3].then { |a| a.map { |x| x * 4 } }
sent = 'ab'.send(:upcase)
[side.first.object_id == out.object_id ? 'self' : 'other', mapped[0], sent]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "self");
    assert_eq!(as_i64(&v[1]), 12);
    assert_eq!(as_str(&v[2]), "AB");
}

#[test]
fn kernel_conversions() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[Integer('42'), Float('2.5'), String(77), Integer(7), Float(3)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 42);
    assert_eq!(as_f64(&v[1]), 2.5);
    assert_eq!(as_str(&v[2]), "77");
    assert_eq!(as_i64(&v[3]), 7);
    assert_eq!(as_f64(&v[4]), 3.0);
    let msg = eval_err(&mut vm, "Integer('zz')");
    assert!(msg.contains("ArgumentError"), "{msg}");
}
