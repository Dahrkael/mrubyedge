use crate::helpers::*;

#[test]
fn ssend0_sends_to_self_with_no_arguments_test() {
    let code = "to_s";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, "main");
}

#[test]
fn send0_sends_to_the_receiver_in_the_register_test() {
    let code = "\"abc\".upcase";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, "ABC");
}

#[test]
fn send0_reaches_a_method_written_in_rust_test() {
    let code = "[1, 2, 3].size";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, 3);
}
