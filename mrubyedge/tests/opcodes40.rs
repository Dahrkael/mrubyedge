extern crate mrubyedge;

mod helpers;
use helpers::*;

use mrubyedge::Error;

const CHUNK: &str = "
class Counter
  def initialize(start)
    @n = start
  end

  def bump(by)
    @n = @n + by
    self
  end

  def value
    @n
  end

  def empty
  end

  def yes
    true
  end

  def no
    false
  end

  def self.build(start)
    new(start)
  end
end

def locals
  a = 1
  b = 2
  c = 3
  a += 4
  b -= 1
  [a, b, c].join(\"-\")
end

def first_of(list)
  list[0]
end

def through_block
  yield 3
end

def bare
  7
end

counter = Counter.build(10)
counter.bump(5)
counter.bump(-3)

parts = [
  counter.value,
  locals,
  first_of([1, 2, 3]),
  through_block { |x| x * 2 },
  bare,
  counter.empty.nil? ? 1 : 0,
  counter.yes ? 1 : 0,
  counter.no ? 0 : 1
]
parts.join(\",\")
";

#[test]
fn getidx0_reads_the_first_element_test() {
    let code = "
a = [5, 6]
a[0]
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 5);
}

#[test]
fn getidx0_asks_an_object_for_its_own_index_method_test() {
    let code = "
class Box
  def [](i)
    i + 100
  end
end

b = Box.new
b[0]
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 100);
}

#[test]
fn ssend0_sends_to_self_with_no_arguments_test() {
    let code = "to_s";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

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
    let result: String = result.as_ref().try_into().unwrap();

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
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 3);
}

#[test]
fn blkcall_calls_the_block_with_the_arguments_test() {
    let code = "
def two
  yield 1, 2
end

two { |a, b| a + b }
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 3);
}

#[test]
fn blkcall_passes_fourteen_arguments_test() {
    // codegen emits BLKCALL only while the argument count fits the nibble (n < 15).
    let code = "
def many
  yield 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14
end

many { |a, b, c, d, e, f, g, h, i, j, k, l, m, n| n }
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 14);
}

#[test]
fn break_out_of_a_blkcalled_block_ends_the_yielding_call_test() {
    let code = "
def counting
  yield 1
  99
end

counting { |x| break x * 2 }
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 2);
}

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
    let result: i64 = result.as_ref().try_into().unwrap();

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
    let result: i64 = result.as_ref().try_into().unwrap();

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
    let result: i64 = result.as_ref().try_into().unwrap();

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
    let result: i64 = result.as_ref().try_into().unwrap();

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
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 1);
}

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

#[test]
fn tdef_defines_the_method_on_the_target_class_test() {
    let code = "
class Greeter
  def hello
    7
  end
end

Greeter.new.hello
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 7);
}

#[test]
fn tdef_leaves_the_method_name_in_the_register_test() {
    let code = "
(def named
  1
end).to_s
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "named");
}

#[test]
fn sdef_defines_a_singleton_method_on_an_object_test() {
    let code = "
o = Object.new
def o.only_mine
  3
end

o.only_mine
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 3);
}

#[test]
fn sdef_defines_a_singleton_method_on_a_class_test() {
    let code = "
class Factory
  def self.build
    2
  end
end

Factory.build
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 2);
}

#[test]
fn sdef_leaves_the_method_name_in_the_register_test() {
    let code = "
o = Object.new
(def o.tagged
  1
end).to_s
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "tagged");
}

#[test]
fn matcherr_passes_when_a_pattern_matched_test() {
    let code = "
x = 1
case x
in 1
  7
end
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: i64 = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, 7);
}

#[test]
fn matcherr_raises_when_nothing_matched_test() {
    let code = "
x = 1
case x
in 2
  7
end
";
    let binary = mrbc_compile("compiled", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let err = vm.run().unwrap_err();
    let err = err.downcast_ref::<Error>().expect("a VM error");

    // Assert
    assert!(
        matches!(err, Error::TaggedError("NoMatchingPatternError", msg) if msg == "pattern not matched"),
        "{:?}",
        err
    );
}

#[test]
fn a_whole_mruby_40_chunk_runs_test() {
    let binary = mrbc_compile("compiled", CHUNK);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "12,5-1-3,1,6,7,1,1,1");
}
