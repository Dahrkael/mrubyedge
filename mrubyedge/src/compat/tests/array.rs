// Tests for the compat Array additions.

use super::{as_bool, as_i64, as_str, as_vec, eval_ok, vm};

#[test]
fn concat_appends_in_place_and_returns_self() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2]\nr = a.concat([3, 4])\n[a.size, a[0], a[1], a[2], a[3], r.object_id == a.object_id ? 'self' : 'other']",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 4);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 2);
    assert_eq!(as_i64(&v[3]), 3);
    assert_eq!(as_i64(&v[4]), 4);
    assert_eq!(as_str(&v[5]), "self");
}

#[test]
fn concat_takes_multiple_arrays_and_keeps_order() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1]\nb = a.concat([2, 3], [4])\n[a.size, b.size, a[1], a[2], a[3]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 4);
    assert_eq!(as_i64(&v[1]), 4);
    assert_eq!(as_i64(&v[2]), 2);
    assert_eq!(as_i64(&v[3]), 3);
    assert_eq!(as_i64(&v[4]), 4);
}

#[test]
fn concat_with_self_appends_a_copy() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2]\na.concat(a)\n[a.size, a[0], a[1], a[2], a[3]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 4);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 2);
    assert_eq!(as_i64(&v[3]), 1);
    assert_eq!(as_i64(&v[4]), 2);
}

#[test]
fn concat_rejects_non_array_arguments() {
    let mut vm = vm();
    let msg = super::eval_err(&mut vm, "[1].concat(2)");
    assert!(msg.contains("ArgumentError"), "{msg}");
}

#[test]
fn fill_overwrites_the_whole_array() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [0, 1, 2, 3, 4]\nb = a.fill(10)\n[a[0], a[4], a.size, b.object_id == a.object_id ? 'self' : 'other']",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 10);
    assert_eq!(as_i64(&v[1]), 10);
    assert_eq!(as_i64(&v[2]), 5);
    assert_eq!(as_str(&v[3]), "self");
}

#[test]
fn fill_start_and_negative_start() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3, 4, 5]\na.fill(9, 2)\nb = a.fill(8, -3)\n[b[0], b[1], b[2], b[3], b[4], [1, 2, 3].fill(7, 9).size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 2);
    assert_eq!(as_i64(&v[2]), 8);
    assert_eq!(as_i64(&v[3]), 8);
    assert_eq!(as_i64(&v[4]), 8);
    assert_eq!(as_i64(&v[5]), 3, "start past the end fills nothing");
}

#[test]
fn fill_start_length_extends_with_nil() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3]\nb = a.fill(9, 2, 5)\nc = [1, 2].fill(7, 1, 2)\n[b[0], b[1], b[2], b[6], b.size, c[0], c[1], c[2], c.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 2);
    assert_eq!(as_i64(&v[2]), 9);
    assert_eq!(as_i64(&v[3]), 9);
    assert_eq!(as_i64(&v[4]), 7);
    assert_eq!(as_i64(&v[5]), 1);
    assert_eq!(as_i64(&v[6]), 7);
    assert_eq!(as_i64(&v[7]), 7);
    assert_eq!(as_i64(&v[8]), 3);
}

#[test]
fn fill_range_inclusive_exclusive_and_endless() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [0, 1, 2, 3, 4]\na.fill(8, 2..4)\nb = [0, 1, 2, 3, 4]\nb.fill(7, 1...3)\nc = [0, 1, 2]\nc.fill(6, 2..)\n[a[0], a[2], a[4], b[1], b[3], c[0], c[2]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 0);
    assert_eq!(as_i64(&v[1]), 8);
    assert_eq!(as_i64(&v[2]), 8);
    assert_eq!(as_i64(&v[3]), 7);
    assert_eq!(as_i64(&v[4]), 3);
    assert_eq!(as_i64(&v[5]), 0);
    assert_eq!(as_i64(&v[6]), 6);
}

#[test]
fn fill_range_beyond_end_extends() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3]\nb = a.fill(9, 2..9)\n[b.size, b[0], b[1], b[2], b[3], b[9]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 10);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 2);
    assert_eq!(as_i64(&v[3]), 9);
    assert_eq!(as_i64(&v[4]), 9);
    assert_eq!(as_i64(&v[5]), 9);
}

#[test]
fn fill_block_form() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [0, 1, 2, 3, 4]\nb = a.fill { |i| i * 100 }\nc = [0, 0, 0, 0, 0]\nc.fill(-2) { |i| i * 10 }\nd = [1, 2, 3, 4]\nd.fill(1..2) { |i| i * 10 }\n[b[0], b[1], b[4], c[0], c[3], c[4], a.fill(1, 2) { |i| i + 1 }[1], d[1], d[3]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 0);
    assert_eq!(as_i64(&v[1]), 100);
    assert_eq!(as_i64(&v[2]), 400);
    assert_eq!(as_i64(&v[3]), 0);
    assert_eq!(as_i64(&v[4]), 30);
    assert_eq!(as_i64(&v[5]), 40);
    assert_eq!(as_i64(&v[6]), 2);
    assert_eq!(as_i64(&v[7]), 10);
    assert_eq!(as_i64(&v[8]), 4);
}

#[test]
fn fill_rejects_negative_length_and_missing_value() {
    let mut vm = vm();
    let msg = super::eval_err(&mut vm, "[1, 2].fill(9, 0, -1)");
    assert!(msg.contains("ArgumentError"), "{msg}");
    let msg2 = super::eval_err(&mut vm, "[1, 2].fill");
    assert!(msg2.contains("ArgumentError"), "{msg2}");
}

#[test]
fn delete_removes_all_matches_and_returns_value() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3, 2]\nremoved = a.delete(2)\n[a.size, a[0], a[1], removed]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 2);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 3);
    assert_eq!(as_i64(&v[3]), 2);
}

#[test]
fn delete_returns_nil_when_absent_and_keeps_array() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = ['x', 'y']\nremoved = a.delete('z')\n[a.size, a[0], removed.nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 2);
    assert_eq!(as_str(&v[1]), "x");
    assert!(as_bool(&v[2]));
}

#[test]
fn index_finds_by_value_and_by_block() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [10, 20, 30]\n[a.index(20), a.index(99).nil?, a.index { |x| x > 15 }, a.find_index(30)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert!(as_bool(&v[1]));
    assert_eq!(as_i64(&v[2]), 1);
    assert_eq!(as_i64(&v[3]), 2);
}

#[test]
fn rindex_scans_from_the_end() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [5, 1, 5, 2]\n[a.rindex(5), a.rindex { |x| x < 3 }, a.rindex(9).nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 2);
    assert_eq!(as_i64(&v[1]), 3);
    assert!(as_bool(&v[2]));
}

#[test]
fn insert_places_and_returns_self() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 4]\nb = a.insert(1, 2)\n[a == b ? 'self' : 'other', a[0], a[1], a[2]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "self");
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 2);
    assert_eq!(as_i64(&v[3]), 4);
}

#[test]
fn insert_negative_and_padding_semantics() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "r = [1, 2].insert(-1, 'end')\np = [1].insert(3, 'x')\n[r[2], p[1].nil?, p.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "end");
    assert!(as_bool(&v[1]));
    assert_eq!(as_i64(&v[2]), 4);
}

#[test]
fn reject_returns_new_filtered_array_and_keeps_original() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3, 4, 5, 6]\nb = a.reject { |x| x % 2 == 0 }\nc = a.reject { |x| false }\n[b[0], b[1], b[2], c.size == 6, a.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 3);
    assert_eq!(as_i64(&v[2]), 5);
    assert!(as_bool(&v[3]));
    assert_eq!(as_i64(&v[4]), 6);
}

#[test]
fn first_and_last_support_arity_and_stay_compatible() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3]\n[a.first, a.first(2)[0], a.first(2)[1], a.first(9).size, a.first(0).size,\n a.last, a.last(2)[0], a.last(2)[1], a.last(9).size, [].first.nil?, [].last.nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 2);
    assert_eq!(as_i64(&v[3]), 3);
    assert_eq!(as_i64(&v[4]), 0);
    assert_eq!(as_i64(&v[5]), 3);
    assert_eq!(as_i64(&v[6]), 2);
    assert_eq!(as_i64(&v[7]), 3);
    assert_eq!(as_i64(&v[8]), 3);
    assert!(as_bool(&v[9]));
    assert!(as_bool(&v[10]));
}

#[test]
fn reverse_variants() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3]\nb = a.reverse\nc = a.reverse!\n[b[0], b.size, c.object_id == a.object_id ? 'self' : 'other', a[0], a[2]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 3);
    assert_eq!(as_i64(&v[1]), 3);
    assert_eq!(as_str(&v[2]), "self");
    assert_eq!(as_i64(&v[3]), 3);
    assert_eq!(as_i64(&v[4]), 1);
}

#[test]
fn reverse_each_walks_tail_first_and_returns_self() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "acc = []\na = [1, 2, 3]\nret = a.reverse_each { |x| acc.push(x) }\n[acc[0], acc[1], acc[2], ret.object_id == a.object_id ? 'self' : 'other']",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 3);
    assert_eq!(as_i64(&v[1]), 2);
    assert_eq!(as_i64(&v[2]), 1);
    assert_eq!(as_str(&v[3]), "self");
}

#[test]
fn rotate_shifts_in_both_directions() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3, 4]\nb = a.rotate\nc = a.rotate(2)\nd = a.rotate(-1)\ne = a.rotate(9)\n[b[0], b[3], c[0], d[0], d[1], e[0]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 2); // rotate => [2,3,4,1]
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 3); // rotate(2) => [3,4,1,2]
    assert_eq!(as_i64(&v[3]), 4); // rotate(-1) => [4,1,2,3]
    assert_eq!(as_i64(&v[4]), 1);
    assert_eq!(as_i64(&v[5]), 2); // rotate(9) == rotate(1)
}

#[test]
fn take_and_drop_slice_the_head() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3, 4]\n[a.take(2)[0], a.take(2).size, a.drop(2)[0], a.drop(2).size, a.take(9).size, a.drop(9).size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 2);
    assert_eq!(as_i64(&v[2]), 3);
    assert_eq!(as_i64(&v[3]), 2);
    assert_eq!(as_i64(&v[4]), 4);
    assert_eq!(as_i64(&v[5]), 0);
}

#[test]
fn take_rejects_negative_counts() {
    let mut vm = vm();
    let msg = super::eval_err(&mut vm, "[1].take(-1)");
    assert!(msg.contains("ArgumentError"), "{msg}");
}

#[test]
fn values_at_supports_negative_and_missing_indices() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [10, 20, 30]\nv = a.values_at(0, 2, -1, 9)\n[v[0], v[1], v[2], v[3].nil?]",
    );
    let got = as_vec(&r);
    assert_eq!(as_i64(&got[0]), 10);
    assert_eq!(as_i64(&got[1]), 30);
    assert_eq!(as_i64(&got[2]), 30);
    assert!(as_bool(&got[3]));
}

#[test]
fn dig_walks_nested_paths_and_short_circuits_on_nil() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "n = [[1, [2, 3]], []]\n[n.dig(0, 1, 1), n.dig(5).nil?, n.dig(0, 5).nil?, n.dig(1, 7).nil?, n.dig(-2, 0)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 3);
    assert!(as_bool(&v[1]));
    assert!(as_bool(&v[2]));
    assert!(as_bool(&v[3]));
    assert_eq!(as_i64(&v[4]), 1);
}

#[test]
fn each_with_object_passes_memo_and_returns_it() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "out = []\nr = [1, 2].each_with_object(out) { |x, m| m.push(x * 2) }\n[r.object_id == out.object_id ? 'same' : 'diff', out[0], out[1], out.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "same");
    assert_eq!(as_i64(&v[1]), 2);
    assert_eq!(as_i64(&v[2]), 4);
    assert_eq!(as_i64(&v[3]), 2);
}

#[test]
fn compact_bang_removes_nils_and_reports_noops() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, nil, 2, nil]\nr1 = a.compact!\nb = [7]\nr2 = b.compact!\n[a[0], a[1], a.size, r1.object_id == a.object_id ? 'self' : 'other', r2.nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 2);
    assert_eq!(as_i64(&v[2]), 2);
    assert_eq!(as_str(&v[3]), "self");
    assert!(as_bool(&v[4]));
}

#[test]
fn product_builds_the_cartesian_tuples() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "p = [1, 2].product([3, 4])\nsolo = [7].product\n[p.size, p[0][0], p[0][1], p[3][0], p[3][1], solo.size, solo[0][0]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 4);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 3);
    assert_eq!(as_i64(&v[3]), 2);
    assert_eq!(as_i64(&v[4]), 4);
    assert_eq!(as_i64(&v[5]), 1);
    assert_eq!(as_i64(&v[6]), 7);
}

#[test]
fn zip_pairs_and_pads_with_nil() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "z = [1, 2].zip([3, 4], [5])\n[z.size, z[0][0], z[0][1], z[0][2], z[1][1], z[1][2].nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 2);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 3);
    assert_eq!(as_i64(&v[3]), 5);
    assert_eq!(as_i64(&v[4]), 4);
    assert!(as_bool(&v[5]));
}

#[cfg(feature = "mruby-random")]
#[test]
fn shuffle_keeps_the_same_multiset() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3, 4]\nb = a.shuffle\n[b.size, b.sort!.join(','), a.join(',')]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 4);
    assert_eq!(as_str(&v[1]), "1,2,3,4");
    assert_eq!(as_str(&v[2]), "1,2,3,4");
}

#[cfg(feature = "mruby-random")]
#[test]
fn sample_picks_members_of_self() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [5, 6, 7]\ns = a.sample\ntwo = a.sample(2)\n[[5, 6, 7].include?(s), two.size, [].sample.nil?]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]));
    assert_eq!(as_i64(&v[1]), 2);
    assert!(as_bool(&v[2]));
}

#[test]
fn reject_bang_removes_in_place_and_reports_noops() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3, 4]\nr1 = a.reject! { |x| x % 2 == 0 }\nb = [1]\nr2 = b.reject! { |x| false }\n[a[0], a[1], r1.object_id == a.object_id ? 'self' : 'other', r2.nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 3);
    assert_eq!(as_str(&v[2]), "self");
    assert!(as_bool(&v[3]));
}

#[test]
fn delete_uses_custom_equality() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        r#"class Box
  def initialize(v)
    @v = v
  end
  # Matches only when this box holds 5, regardless of the argument.
  def ==(other)
    @v == 5
  end
  def tag
    @v
  end
end
a = [Box.new(1), Box.new(5)]
removed = a.delete(Box.new(99))
[a.size, a[0].tag, removed.nil?]"#,
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 1);
    assert!(!as_bool(&v[2]));
}

#[test]
fn delete_tolerates_equality_mutating_the_array() {
    // The element's == clears the receiver: Array#delete must not hold a
    // borrow across the user callback (this panicked with BorrowMutError).
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        r#"class Mutator
  def initialize(arr)
    @arr = arr
  end
  def ==(other)
    @arr.clear
    false
  end
end
a = []
a.push(Mutator.new(a))
a.push(1)
a.delete(99)
a.size"#,
    );
    assert_eq!(as_i64(&r), 0);
}

#[test]
fn new_with_block_fills_each_position() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = Array.new(3) { |i| i * 10 }\n[a[0], a[1], a[2], a.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 0);
    assert_eq!(as_i64(&v[1]), 10);
    assert_eq!(as_i64(&v[2]), 20);
    assert_eq!(as_i64(&v[3]), 3);
}

#[test]
fn new_without_block_fills_nil_or_default() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = Array.new(3)\nb = Array.new(2, 'x')\nempty = Array.new\n[a[0].nil?, a[1].nil?, a[2].nil?, b[0], b[1], empty.size]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]));
    assert!(as_bool(&v[1]));
    assert!(as_bool(&v[2]));
    assert_eq!(as_str(&v[3]), "x");
    assert_eq!(as_str(&v[4]), "x");
    assert_eq!(as_i64(&v[5]), 0);
}

#[test]
fn new_rejects_negative_size() {
    let mut vm = vm();
    let msg = super::eval_err(&mut vm, "Array.new(-1)");
    assert!(msg.contains("ArgumentError"), "{msg}");
}

#[test]
fn get_index_out_of_range_returns_nil() {
    let mut vm = vm();
    let r = eval_ok(&mut vm, "a = [1, 2]\n[a[5].nil?, a[-9].nil?, a[0], a[-1]]");
    let v = as_vec(&r);
    assert!(as_bool(&v[0]));
    assert!(as_bool(&v[1]));
    assert_eq!(as_i64(&v[2]), 1);
    assert_eq!(as_i64(&v[3]), 2);
}

// A native method that calls back into Ruby (delete -> rb_eq -> funcall) must
// not clobber the caller's upvar chain for enclosing blocks.
#[test]
fn native_ruby_call_preserves_upvar_chain() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        r#"class Box
  def ==(other)
    true
  end
end
class Grid
  def build
    base = 5
    acc = []
    2.times do |y|
      [Box.new].delete(Box.new)
      2.times { |x| acc.push(base) }
    end
    acc.join(',')
  end
end
Grid.new.build"#,
    );
    assert_eq!(as_str(&r), "5,5,5,5");
}

#[test]
fn native_ruby_call_preserves_upvar_chain_for_assignment() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        r#"class Box
  def ==(other)
    true
  end
end
class Counter
  def build
    total = 0
    2.times do |y|
      [Box.new].delete(Box.new)
      2.times { |x| total += 1 }
    end
    total
  end
end
Counter.new.build"#,
    );
    assert_eq!(as_i64(&r), 4);
}

#[test]
fn spaceship_orders_lexicographically() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "
a = [1, 2] <=> [1, 3]
b = [1, 3] <=> [1, 2]
c = [1, 2] <=> [1, 2]
d = [1] <=> [1, 2]
e = [1, 2] <=> [1]
f = [1, 2] <=> 5
g = [1, 2] <=> 'x'
[a, b, c, d, e, f.nil?, g.nil?]
",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), -1);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 0);
    assert_eq!(as_i64(&v[3]), -1, "shorter array sorts first");
    assert_eq!(as_i64(&v[4]), 1, "longer array sorts last");
    assert!(as_bool(&v[5]), "non-array other side is nil");
    assert!(as_bool(&v[6]), "non-array other side is nil");
}

#[test]
fn sort_by_accepts_array_keys() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "
data = [[3, 2], [1, 9], [3, 1], [2, 5]]
sorted = data.sort_by { |d| [d[0], d[1]] }
sorted.map { |d| d.join(',') }
",
    );
    let v = as_vec(&r);
    let joined: Vec<String> = v.iter().map(as_str).collect();
    assert_eq!(joined, vec!["1,9", "2,5", "3,1", "3,2"]);
}
