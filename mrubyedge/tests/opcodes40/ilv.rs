use crate::helpers::*;
use mrubyedge::Error;

#[test]
fn addilv_adds_the_immediate_to_an_integer_local_test() {
    let code = "
x = 1
x += 5
x
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 6);
}

#[test]
fn subilv_subtracts_the_immediate_from_an_integer_local_test() {
    let code = "
x = 9
x -= 3
x
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 6);
}

#[test]
fn addilv_adds_to_a_float_local_test() {
    let code = "
x = 1.5
x += 2
x
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: f64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 3.5);
}

#[test]
fn addilv_sends_plus_to_anything_else_test() {
    let code = "
class Tally
  def +(n)
    n + 100
  end
end

x = Tally.new
x += 5
x
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 105);
}

#[test]
fn subilv_sends_minus_to_anything_else_test() {
    let code = "
class Tally
  def -(n)
    n + 100
  end
end

x = Tally.new
x -= 5
x
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 105);
}

#[test]
fn addilv_leaves_the_other_locals_alone_when_it_sends_test() {
    let code = "
class Tally
  def +(n)
    n + 100
  end
end

a = 1
b = Tally.new
c = 3
b += 5
[a, b, c].join(\"-\")
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "1-105-3");
}

#[test]
fn addilv_on_an_object_without_plus_is_a_no_method_error_test() {
    let code = "
class Bare
end

x = Bare.new
x += 1
x
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let err = vm.run().unwrap_err();
    let err = err.downcast_ref::<Error>().expect("a VM error");

    // Assert
    assert!(matches!(err, Error::NoMethodError(_)), "{:?}", err);
}
