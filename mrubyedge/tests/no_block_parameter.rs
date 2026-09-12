extern crate mrubyedge;

mod helpers;
use helpers::*;

use mrubyedge::Error;

#[test]
fn a_method_that_refuses_a_block_runs_without_one_test() {
    let code = "
def alone(&nil)
  1
end

alone
";
    let binary = mrbc_compile("no_block_none_given", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 1);
}

#[test]
fn a_method_that_refuses_a_block_rejects_one_test() {
    let code = "
def alone(&nil)
  1
end

alone { 2 }
";
    let binary = mrbc_compile("no_block_given", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let err = vm.run().unwrap_err();
    let err = err.downcast_ref::<Error>().expect("a VM error");

    // Assert
    assert!(
        matches!(err, Error::ArgumentError(msg) if msg == "no block accepted"),
        "{:?}",
        err
    );
}

#[test]
fn a_method_that_takes_a_block_still_takes_one_test() {
    let code = "
def with(&b)
  b.call
end

with { 3 }
";
    let binary = mrbc_compile("no_block_control", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 3);
}
