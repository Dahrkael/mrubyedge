// Tests for the compat Hash additions.

use super::{as_bool, as_i64, as_str, as_vec, eval_err, eval_ok, vm};

#[test]
fn fetch_supports_default_block_and_key_error() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "h = {'a' => 1}\n[h.fetch('a'), h.fetch('z', 9), h.fetch('q') { |k| k.upcase }]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 9);
    assert_eq!(as_str(&v[2]), "Q");
    let msg = eval_err(&mut vm, "{'a' => 1}.fetch('zz')");
    assert!(msg.contains("KeyError"), "{msg}");
}

#[test]
fn key_member_value_predicates() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "h = {'a' => 1}\n[h.key?('a'), h.member?('a'), h.key?('nope'), h.value?(1), h.value?(2)]",
    );
    let v = as_vec(&r);
    for i in [0usize, 1, 3] {
        assert!(as_bool(&v[i]), "index {i}");
    }
    for i in [2usize, 4] {
        assert!(!as_bool(&v[i]), "index {i}");
    }
}

#[test]
fn dig_walks_hashes_and_nested_containers() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "h = {'a' => [1, {'k' => 5}]}\n[h.dig('a', 0), h.dig('a', 1, 'k'), h.dig('zz').nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 5);
    assert!(as_bool(&v[2]));
}

#[test]
fn values_at_fills_missing_with_nil() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "h = {'a' => 1}\nv = h.values_at('a', 'nope')\n[v[0], v[1].nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert!(as_bool(&v[1]));
}

#[test]
fn transform_values_and_keys() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "h = {'a' => 1, 'b' => 2}\nt = h.transform_values { |x| x * 10 }\ntk = h.transform_keys { |k| k.upcase }\n[t['a'], t['b'], tk['A'], tk['B'], h['a']]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 10);
    assert_eq!(as_i64(&v[1]), 20);
    assert_eq!(as_i64(&v[2]), 1);
    assert_eq!(as_i64(&v[3]), 2);
    assert_eq!(as_i64(&v[4]), 1);
}

#[test]
fn invert_swaps_pairs_and_store_inserts() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "inv = {'a' => 7}.invert\nhh = {}\nr = hh.store('x', 3)\n[inv[7], hh['x'], r]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "a");
    assert_eq!(as_i64(&v[1]), 3);
    assert_eq!(as_i64(&v[2]), 3);
}

#[test]
fn keep_if_filters_in_place() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "h = {'a' => 1, 'bb' => 2, 'c' => 3}\nr = h.keep_if { |k, v| k.size == 1 }\n[r.object_id == h.object_id ? 'self' : 'other', r.size, r['bb'].nil?]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "self");
    assert_eq!(as_i64(&v[1]), 2);
    assert!(as_bool(&v[2]));
}

#[test]
fn merge_variants_and_conflict_block() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = {'x' => 1}\nb = a.merge({'y' => 2})\nc = a.merge!({'x' => 9})\nm = {'x' => 1}.merge({'x' => 10}) { |k, o, n| o + n }\n[b['x'], b['y'], c['x'], m['x']]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 1);
    assert_eq!(as_i64(&v[1]), 2);
    assert_eq!(as_i64(&v[2]), 9);
    assert_eq!(as_i64(&v[3]), 11);
}
