extern crate mrubyedge;

mod helpers;
use helpers::*;

#[test]
fn numeric_comparisons_still_native_test() {
    let code = r#"
    [(1 < 2), (2 <= 2), (3 > 2.5), (2 >= 3), (1.5 < 2)]
    "#;
    let binary = mrbc_compile("numeric_comparisons", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let arr: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        result.as_ref().try_into().unwrap();
    let vals: Vec<bool> = arr.iter().map(|r| r.as_ref().try_into().unwrap()).collect();
    assert_eq!(vals, vec![true, true, true, false, true]);
}

#[test]
fn string_comparison_via_spaceship_test() {
    let code = r#"
    [("a" < "b"), ("b" <= "a"), ("b" > "a"), ("a" >= "b")]
    "#;
    let binary = mrbc_compile("string_comparison", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let arr: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        result.as_ref().try_into().unwrap();
    let vals: Vec<bool> = arr.iter().map(|r| r.as_ref().try_into().unwrap()).collect();
    assert_eq!(vals, vec![true, false, true, false]);
}

#[test]
fn user_defined_spaceship_drives_comparisons_test() {
    let code = r#"
    class Version
      def initialize(n)
        @n = n
      end

      def <=>(other)
        @n <=> other.n
      end

      def n
        @n
      end
    end

    [(Version.new(1) < Version.new(2)), (Version.new(2) >= Version.new(2)), (Version.new(3) > Version.new(2))]
    "#;
    let binary = mrbc_compile("version_comparison", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let arr: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        result.as_ref().try_into().unwrap();
    let vals: Vec<bool> = arr.iter().map(|r| r.as_ref().try_into().unwrap()).collect();
    assert_eq!(vals, vec![true, true, true]);
}

#[test]
fn incomparable_operands_raise_instead_of_panicking_test() {
    let code = r#"
    class Empty
    end
    Empty.new < Empty.new
    "#;
    let binary = mrbc_compile("incomparable", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    assert!(
        vm.run().is_err(),
        "comparison without <=> must raise, not panic"
    );
}
