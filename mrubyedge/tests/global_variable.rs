extern crate mrubyedge;

mod helpers;
use helpers::*;

#[test]
fn a_global_variable_keeps_what_was_assigned_test() {
    let code = "
    $answer = 42
    $answer
    ";
    let binary = mrbc_compile("global_assigned", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 42);
}

#[test]
fn a_global_variable_assigned_in_a_method_is_visible_outside_test() {
    let code = "
    def remember
      $note = 7
    end

    remember
    $note
    ";
    let binary = mrbc_compile("global_in_method", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 7);
}

#[test]
fn a_global_variable_that_was_never_assigned_is_nil_test() {
    let code = "
    $never_set.nil? ? 1 : 0
    ";
    let binary = mrbc_compile("global_unset", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 1);
}

#[test]
fn a_global_variable_that_was_never_assigned_answers_like_nil_test() {
    let code = "
    $never_set.to_s
    ";
    let binary = mrbc_compile("global_unset_to_s", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "");
}
