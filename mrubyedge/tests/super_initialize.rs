// Regression tests for the funcall-callinfo fix: `super` with arguments must
// dispatch from any method, including `initialize` invoked through
// Class#new (mrb_funcall/call_block). Before the fix the callee ran with a
// hidden current_callinfo, so op_super failed with "no current callinfo".

extern crate mrubyedge;

mod helpers;
use helpers::*;

#[test]
fn super_with_args_in_initialize() {
    let code = r#"
class A1
  def initialize(x)
    @x = x
  end
  def value
    @x
  end
end

class B1 < A1
  def initialize(x)
    super(x)
  end
end

B1.new(42).value
    "#;
    let binary = mrbc_compile("super_with_args_in_initialize", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 42);
}

#[test]
fn super_with_optional_arg_in_initialize() {
    let code = r#"
class Parent
  def initialize(x = nil)
    @value = x
  end
  def value
    @value
  end
end

class Child < Parent
  def initialize(x = nil)
    super(x)
  end
end

Child.new(7).value
    "#;
    let binary = mrbc_compile("super_with_optional_arg_in_initialize", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 7);
}

#[test]
fn super_in_regular_method() {
    let code = r#"
class Base
  def double(x)
    x * 2
  end
end

class Derived < Base
  def double(x)
    super(x) + 1
  end
end

Derived.new.double(21)
    "#;
    let binary = mrbc_compile("super_in_regular_method", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 43);
}

#[test]
fn super_chain_through_initialize() {
    let code = r#"
class A
  def initialize(v)
    @v = v
  end
  def v
    @v
  end
end

class B < A
  def initialize(v)
    super(v)
    @v = @v + 1
  end
end

class C < B
  def initialize(v)
    super(v)
    @v = @v + 1
  end
end

C.new(10).v
    "#;
    let binary = mrbc_compile("super_chain_through_initialize", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 12);
}

#[test]
fn super_value_passthrough_from_method() {
    let code = r##"
class Base
  def greet(name)
    "hello #{name}"
  end
end

class Derived < Base
  def greet(name)
    "#{super(name)}!"
  end
end

Derived.new.greet("mundo")
    "##;
    let binary = mrbc_compile("super_value_passthrough_from_method", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_str: String = result.try_into().unwrap();
    assert_eq!(result_str, "hello mundo!");
}

#[test]
fn super_after_nested_send_from_initialize() {
    let code = r#"
class Helper
  def bump(v)
    v + 1
  end
end

class Base
  def initialize(v)
    @value = v
  end
  def value
    @value
  end
end

class Child < Base
  def initialize(v)
    @helper = Helper.new
    super(@helper.bump(v))
  end
end

Child.new(10).value
    "#;
    let binary = mrbc_compile("super_after_nested_send_from_initialize", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 11);
}

#[test]
fn bare_super_forwards_initialize_args() {
    let code = r#"
class Parent
  def initialize(x, y)
    @x = x
    @y = y
  end
  def sum
    @x + @y
  end
end

class Child < Parent
  def initialize(x, y)
    super
  end
end

Child.new(20, 22).sum
    "#;
    let binary = mrbc_compile("bare_super_forwards_initialize_args", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 42);
}

#[test]
fn bare_super_forwards_method_args() {
    let code = r#"
class Base
  def scale(x, y)
    x * y
  end
end

class Derived < Base
  def scale(x, y)
    super
  end
end

Derived.new.scale(6, 7)
    "#;
    let binary = mrbc_compile("bare_super_forwards_method_args", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 42);
}

#[test]
fn bare_super_chain_through_initialize() {
    let code = r#"
class A
  def initialize(x)
    @x = x
  end
  def value
    @x
  end
end

class B < A
  def initialize(x)
    super
    @x = @x + 1
  end
end

class C < B
  def initialize(x)
    super
    @x = @x + 1
  end
end

C.new(40).value
    "#;
    let binary = mrbc_compile("bare_super_chain_through_initialize", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 42);
}

#[test]
fn bare_super_with_rest_args() {
    let code = r#"
class Base
  def combine(*items)
    items[0] + items[1] + items[2]
  end
end

class Derived < Base
  def combine(*items)
    super
  end
end

Derived.new.combine(10, 20, 12)
    "#;
    let binary = mrbc_compile("bare_super_with_rest_args", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result_int: i32 = result.try_into().unwrap();
    assert_eq!(result_int, 42);
}
