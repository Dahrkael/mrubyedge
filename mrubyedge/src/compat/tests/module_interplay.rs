// Locks the interaction between the compat layer's native-on-module methods
// and mrubyedge's canonical RModule wrappers after a Ruby-side reopen.

use super::{as_f64, as_i64, eval_ok, vm};

#[test]
fn math_keeps_native_singletons_after_ruby_reopen() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "module Math\n  def self.ping\n    7\n  end\nend\n[Math.sqrt(4), Math.ping]",
    );
    let v = super::as_vec(&r);
    assert_eq!(as_f64(&v[0]), 2.0);
    assert_eq!(as_i64(&v[1]), 7);
}

#[test]
fn enumerable_survives_ruby_extension() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "module Enumerable\n  def triple\n    map { |x| x * 3 }\n  end\nend\n[1, 2].triple.join(',')",
    );
    assert_eq!(super::as_str(&r), "3,6");
}

#[test]
fn comparable_still_mixed_after_reopen() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "class Rank\n  include Comparable\n  def initialize(v)\n    @v = v\n  end\n  attr_reader :v\n  def <=>(o)\n    @v <=> o.v\n  end\nend\nmodule Comparable\n  def banner\n    'ranked'\n  end\nend\n[Rank.new(2) < Rank.new(5), Rank.new(1).banner]",
    );
    let v = super::as_vec(&r);
    assert!(super::as_bool(&v[0]));
    assert_eq!(super::as_str(&v[1]), "ranked");
}
