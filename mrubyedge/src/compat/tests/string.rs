// Tests for String#% and the Kernel format entry points.

use super::{as_bool, as_i64, as_str, as_vec, eval_ok, vm};

#[test]
fn tr_maps_ranges_and_delete_squeeze_count_sets() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "['hello'.tr('el', 'ip'), 'a-b-c'.tr('a-c', 'xyz'), 'hello'.delete('l'), 'bookkeeper'.squeeze(), 'aaabbc'.squeeze('a'), 'mississippi'.count('is')]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "hippo");
    assert_eq!(as_str(&v[1]), "x-y-z");
    assert_eq!(as_str(&v[2]), "heo");
    assert_eq!(as_str(&v[3]), "bokeper");
    assert_eq!(as_str(&v[4]), "abbc");
    assert_eq!(as_i64(&v[5]), 8);
}

#[test]
fn gsub_and_sub_replace_literal_patterns() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "['hello world'.gsub('o', '0'), 'banana'.gsub('na', 'NA'), 'hello'.sub('l', 'L'), 'same'.gsub('zz', 'q')]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "hell0 w0rld");
    assert_eq!(as_str(&v[1]), "baNANA");
    assert_eq!(as_str(&v[2]), "heLlo");
    assert_eq!(as_str(&v[3]), "same");
}

#[test]
fn alignment_and_case_helpers() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "['ab'.center(7), 'ab'.center(6, '*'), 'hi'.ljust(4, '.'), 'hi'.rjust(4, '.'), 'Hello'.swapcase(), 'abc'.casecmp('ABC'), 'abc'.casecmp?('ABD'), 'hello'.reverse]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "  ab   ");
    assert_eq!(as_str(&v[1]), "**ab**");
    assert_eq!(as_str(&v[2]), "hi..");
    assert_eq!(as_str(&v[3]), "..hi");
    assert_eq!(as_str(&v[4]), "hELLO");
    assert_eq!(as_i64(&v[5]), 0);
    assert!(!as_bool(&v[6]));
    assert_eq!(as_str(&v[7]), "olleh");
}

#[test]
fn destructive_edits_and_partition() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "s = 'mid'\ns.prepend('pre-')\ns.concat('-post')\nt = 'x'\nt.replace('y')\np = 'key=value'.partition('=')\nch = 'hi!'.chop\n[s, t, p[0], p[2], ch]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "pre-mid-post");
    assert_eq!(as_str(&v[1]), "y");
    assert_eq!(as_str(&v[2]), "key");
    assert_eq!(as_str(&v[3]), "value");
    assert_eq!(as_str(&v[4]), "hi");
}

#[test]
fn iteration_hex_oct_and_succ() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "chars = []\n'ab'.each_char { |c| chars.push(c) }\nbytes = []\n'AB'.each_byte { |b| bytes.push(b) }\nlines = \"x\\ny\".lines\n['0xFF'.hex, '10'.oct, 'az'.succ, 'z'.succ, '9'.succ, lines.size, lines[1], chars.join(','), bytes[0]]",
    );
    let v = as_vec(&r);
    assert_eq!(as_i64(&v[0]), 255);
    assert_eq!(as_i64(&v[1]), 8);
    assert_eq!(as_str(&v[2]), "ba");
    assert_eq!(as_str(&v[3]), "aa");
    assert_eq!(as_str(&v[4]), "10");
    assert_eq!(as_i64(&v[5]), 2);
    assert_eq!(as_str(&v[6]), "y");
    assert_eq!(as_str(&v[7]), "a,b");
    assert_eq!(as_i64(&v[8]), 65);
}

#[test]
fn gsub_block_receives_each_match() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "['aaa'.gsub('a') { |m| m.upcase }, 'a b'.gsub(' ') { '-' }, 'xy'.gsub('z') { 'never' }]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "AAA");
    assert_eq!(as_str(&v[1]), "a-b");
    assert_eq!(as_str(&v[2]), "xy");
}

#[test]
fn percent_formats_integers_and_strings() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "['%d!' % 42, '%s x %d' % ['a', 3], '%-5s|' % 'ab', '%5s|' % 'ab']",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "42!");
    assert_eq!(as_str(&v[1]), "a x 3");
    assert_eq!(as_str(&v[2]), "ab   |");
    assert_eq!(as_str(&v[3]), "   ab|");
}

#[test]
fn float_precision_zero_pad_and_bases() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "['%.2f' % 3.14159, '%05d' % 42, '%x %o %b' % [255, 8, 5], '%+d' % 5, '%c' % 65]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "3.14");
    assert_eq!(as_str(&v[1]), "00042");
    assert_eq!(as_str(&v[2]), "ff 10 101");
    assert_eq!(as_str(&v[3]), "+5");
    assert_eq!(as_str(&v[4]), "A");
}

#[test]
fn kernel_sprintf_and_format_match() {
    let mut vm = vm();
    let r = eval_ok(
        &mut vm,
        "[sprintf('%08.3f', 3.14159), format('%e', 150.0), sprintf('%X', 255)]",
    );
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "0003.142");
    assert_eq!(as_str(&v[1]), "1.500000e+02");
    assert_eq!(as_str(&v[2]), "FF");
}

#[test]
fn format_negative_zero_pad_keeps_full_width() {
    let mut vm = vm();
    let r = eval_ok(&mut vm, "[sprintf('%08.3f', -3.14159), '%06d' % -42]");
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "-003.142");
    assert_eq!(as_str(&v[1]), "-00042");
}

#[test]
fn format_precision_truncates_by_character() {
    let mut vm = vm();
    let r = eval_ok(&mut vm, "[sprintf('%.1s', 'é'), sprintf('%.2s', 'héllo')]");
    let v = as_vec(&r);
    assert_eq!(as_str(&v[0]), "é");
    assert_eq!(as_str(&v[1]), "hé");
}

#[test]
fn format_c_string_takes_first_char() {
    let mut vm = vm();
    assert_eq!(as_str(&eval_ok(&mut vm, "sprintf('%c', 'abc')")), "a");
    let msg = super::eval_err(&mut super::vm(), "sprintf('%c', '')");
    assert!(msg.contains("ArgumentError"), "{msg}");
}

#[test]
fn format_huge_width_is_clamped() {
    let mut vm = vm();
    let s = as_str(&eval_ok(&mut vm, "sprintf('%99999999999999999999d', 1)"));
    assert_eq!(s.len(), 1_000_000, "width saturates instead of overflowing");
}

#[test]
fn reverse_is_utf8_safe() {
    let mut vm = vm();
    assert_eq!(as_str(&eval_ok(&mut vm, "'héllo'.reverse")), "olléh");
}

#[test]
fn negative_width_raises() {
    let msg = super::eval_err(&mut vm(), "'a'.ljust(-1)");
    assert!(msg.contains("ArgumentError"), "{msg}");
    let msg = super::eval_err(&mut vm(), "'a'.rjust(-1)");
    assert!(msg.contains("ArgumentError"), "{msg}");
}
