// Tests for the compat Enumerable additions.

use super::{as_bool, as_f64, as_i64, as_str, as_vec, eval_ok, vm};

#[test]
fn array_reject_filter_map_variants() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [1, 2, 3, 4]\nrj = a.reject { |x| x % 2 == 0 }\nfm = a.filter_map { |x| x * 3 if x > 2 }\nfl = a.flat_map { |x| [x, -x] }\n[rj.join(','), fm.join(','), fl.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "1,3");
    assert_eq!(as_str(&v[1]), "9,12");
    assert_eq!(as_i64(&v[2]), 8);
}

#[test]
fn none_one_partition_tally() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "a = [2, 4, 5]\npt = a.partition { |x| x % 2 == 0 }\ntl = %w[a b a].tally\n[a.none? { |x| x > 10 }, a.one? { |x| x.odd? }, pt[0].size, tl['a']]",
    );
    let v = as_vec(&r);
    assert!(as_bool(&v[0]));
    assert!(as_bool(&v[1]));
    assert_eq!(as_i64(&v[2]), 2);
    assert_eq!(as_i64(&v[3]), 2);
}

#[test]
fn max_by_min_by_and_slices() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "w = %w[gold silver bronze]\nsl = []\n(1..6).each_slice(2) { |s| sl.push(s) }\ncs = []\n(1..4).each_cons(2) { |s| cs.push(s) }\n[w.max_by { |x| x.size }, w.min_by { |x| x.size }, sl.size, cs.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "silver");
    assert_eq!(as_str(&v[1]), "gold");
    assert_eq!(as_i64(&v[2]), 3);
    assert_eq!(as_i64(&v[3]), 3);
}

#[test]
fn take_while_drop_while_on_range() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "rng = (1..10)\n[rng.take_while { |x| x < 4 }.join(','), rng.drop_while { |x| x < 4 }.join(','), rng.filter { |x| x.even? }.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "1,2,3");
    assert_eq!(as_str(&v[1]), "4,5,6,7,8,9,10");
    assert_eq!(as_i64(&v[2]), 5);
}

#[test]
fn group_by_buckets() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "g = (1..6).group_by { |x| x % 2 }\n[g[0].size, g[1][0], g[1][2]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 3);
    assert_eq!(as_i64(&v[1]), 1);
    assert_eq!(as_i64(&v[2]), 5);
}

#[test]
fn hash_enumerable_yields_pairs() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "h = {'a' => 1, 'bb' => 2}\nmx = h.max_by { |k, v| v }\nrj = h.reject { |k, v| v == 1 }\n[mx[0], mx[1], rj.size]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "bb");
    assert_eq!(as_f64(&v[1]), 2.0);
    assert_eq!(as_i64(&v[2]), 1);
}
