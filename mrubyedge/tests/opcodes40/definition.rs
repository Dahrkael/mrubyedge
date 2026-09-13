use crate::helpers::*;

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
    let result: i64 = (&result).try_into().unwrap();

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
    let result: String = (&result).try_into().unwrap();

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
    let result: i64 = (&result).try_into().unwrap();

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
    let result: i64 = (&result).try_into().unwrap();

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
    let result: String = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, "tagged");
}
