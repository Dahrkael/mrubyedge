use crate::helpers::*;

#[test]
fn retself_returns_the_receiver_test() {
    let code = "
class Counter
  def initialize
    @n = 5
  end

  def itself_again
    self
  end

  def n
    @n
  end
end

Counter.new.itself_again.n
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, 5);
}

#[test]
fn retself_is_not_the_last_value_computed_test() {
    let code = "
class Counter
  def compute_then_self
    1 + 1
    self
  end

  def tag
    42
  end
end

Counter.new.compute_then_self.tag
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, 42);
}

#[test]
fn retnil_returns_nil_test() {
    let code = "
def empty
end

empty.nil? ? 1 : 0
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, 1);
}

#[test]
fn rettrue_returns_true_test() {
    let code = "
def yes
  true
end

yes ? 1 : 0
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, 1);
}

#[test]
fn retfalse_returns_false_test() {
    let code = "
def no
  false
end

no ? 0 : 1
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, 1);
}
