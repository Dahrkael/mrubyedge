extern crate mec_mrbc_sys;
extern crate mrubyedge;

mod helpers;
use helpers::*;

#[test]
fn module_definition_test() {
    let script = r#"
module TestModule
  def module_method
    42
  end
end

TestModule
"#;

    let binary = mrbc_compile("module_def", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    // Result should be the module itself
    assert!(matches!(
        result.to_rc().tt,
        mrubyedge::yamrb::value::RType::Module
    ));
}

#[test]
fn class_can_include_module_method() {
    let script = r#"
module Printable
  def greet
    "hello"
  end
end

class User
  include Printable
end

User.new.greet
"#;

    let binary = mrbc_compile("module_include", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    let value: String = result.try_into().expect("greet should return string");
    assert_eq!(value, "hello");
}

#[test]
fn modules_can_be_used_as_namespace() {
    let script = r#"
module Outer
  module Printable
    def greet
      "hello"
    end
  end

  class User
    include Printable
  end
end

Outer::User.new.greet
"#;

    let binary = mrbc_compile("module_include_ns", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    let value: String = result.try_into().expect("greet should return string");
    assert_eq!(value, "hello");
}

#[test]
fn module_can_include_module_method() {
    let script = r#"
module Core
  def core_value
    123
  end
end

module Superset
  include Core
end

class Wrapper
  include Superset
end

Wrapper.new.core_value
"#;

    let binary = mrbc_compile("module_include_module", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    let value: i64 = result.try_into().expect("core_value should return integer");
    assert_eq!(value, 123);
}

#[test]
fn module_can_include_module_method_2() {
    let script = r#"
module Core
  def core_value
    123
  end
end

module Superset
  include Core

  def core_value
    super + 1
  end
end

class Wrapper
  include Superset
end

Wrapper.new.core_value
"#;

    let binary = mrbc_compile("module_include_module_2", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    let value: i64 = result.try_into().expect("core_value should return integer");
    assert_eq!(value, 124);
}

#[test]
fn constants_inside_module_are_scoped_to_the_module() {
    let script = r#"
module CptnTiles
  Grass = 0
  Earth = 1
end
CptnTiles::Earth
"#;

    let binary = mrbc_compile("module_const_scoped", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    let value: i64 = result
        .try_into()
        .expect("CptnTiles::Earth should return an integer");
    assert_eq!(value, 1);
}

#[test]
fn nested_module_constants_resolve_through_the_path() {
    let script = r#"
module Outer
  module Inner
    V = 7
  end
end
Outer::Inner::V
"#;

    let binary = mrbc_compile("module_const_nested", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    let value: i64 = result
        .try_into()
        .expect("Outer::Inner::V should return an integer");
    assert_eq!(value, 7);
}

#[test]
fn class_constants_resolve_qualified() {
    let script = r#"
class C
  W = 10
end
C::W
"#;

    let binary = mrbc_compile("class_const_qualified", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    let value: i64 = result.try_into().expect("C::W should return an integer");
    assert_eq!(value, 10);
}

#[test]
fn class_constants_stay_readable_from_instance_methods() {
    // op_getconst falls back to the runtime class of self when there is no
    // namespace (instance method), so constants defined in a class body
    // remain reachable from its own methods.
    let script = r#"
class C
  W = 10
  def read
    W
  end
end
C.new.read
"#;

    let binary = mrbc_compile("class_const_from_method", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();

    let value: i64 = result.try_into().expect("read should return an integer");
    assert_eq!(value, 10);
}

#[test]
fn class_constants_do_not_leak_to_top_level_bare_reads() {
    // Scoping to the class means a bare read at top level no longer resolves
    // the class constant from the global table (matches real Ruby).
    let script = r#"
class C
  X = 1
end
X
"#;

    let binary = mrbc_compile("class_const_no_leak", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    assert!(
        vm.run().is_err(),
        "bare top-level X should not resolve C::X"
    );
}

// KNOWN LIMITATION, not worth fixing by design: real Ruby binds a constant
// assigned inside an instance method to its lexical class via the cref, so
// Foo::THRESHOLD == 10 here. mrubyedge has no cref: op_setconst derives the
// namespace from `self`, and inside an instance method self is an instance,
// so the assignment falls back to the global table and Foo::THRESHOLD raises
// NameError. Full lexical constant scope would need cref support across the
// VM; this engine keeps the pragmatic global fallback instead.
#[test]
fn instance_method_constant_assignment_is_not_lexically_scoped() {
    let script = r#"
class Foo
  def setup
    THRESHOLD = 10
  end
end
Foo.new.setup
Foo::THRESHOLD
"#;

    let binary = mrbc_compile("inst_method_const_limitation", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    assert!(
        vm.run().is_err(),
        "Foo::THRESHOLD should fail (documented limitation)"
    );
}

// KNOWN LIMITATION, not worth fixing by design: a bare constant read inside
// an instance method resolves through the runtime class of self, but the walk
// follows the module nesting (`.parent`) chain, not the superclass chain.
// Real Ruby reaches Base::CONFIG lexically; here Child.new.r raises NameError.
#[test]
fn superclass_constants_are_not_walked_in_bare_reads() {
    let script = r#"
class Base
  CONFIG = 5
end

class Child < Base
  def r
    CONFIG
  end
end

Child.new.r
"#;

    let binary = mrbc_compile("superclass_const_limitation", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    assert!(
        vm.run().is_err(),
        "Child.new.r should fail (documented limitation)"
    );
}

// KNOWN LIMITATION, not worth fixing by design: a bare constant read inside
// a method provided by an included module resolves through the runtime class
// of self, which does not reach the included module's consts. Real Ruby finds
// M::X via the lexical cref; here C.new.m raises NameError.
#[test]
fn included_module_constants_are_not_walked_in_bare_reads() {
    let script = r#"
module M
  X = 1
  def m
    X
  end
end

class C
  include M
end

C.new.m
"#;

    let binary = mrbc_compile("included_module_const_limitation", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    assert!(
        vm.run().is_err(),
        "C.new.m should fail (documented limitation)"
    );
}

#[test]
fn setmcnst_module_const_test() {
    let code = r#"
    module M
    end
    M::X = 42
    def setmcnst_module_const
      M::X
    end
    "#;
    let binary = mrbc_compile("setmcnst_module_const", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "setmcnst_module_const", &args).unwrap();
    let result: i64 = result.try_into().unwrap();
    assert_eq!(result, 42);
}

#[test]
fn setmcnst_top_level_const_test() {
    let code = r#"
    ::G = 7
    def setmcnst_top_level
      G
    end
    "#;
    let binary = mrbc_compile("setmcnst_top_level", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "setmcnst_top_level", &args).unwrap();
    let result: i64 = result.try_into().unwrap();
    assert_eq!(result, 7);
}

#[test]
fn setmcnst_overwrite_test() {
    let code = r#"
    module N
    end
    N::X = 1
    N::X = 2
    def setmcnst_overwrite
      N::X
    end
    "#;
    let binary = mrbc_compile("setmcnst_overwrite", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "setmcnst_overwrite", &args).unwrap();
    let result: i64 = result.try_into().unwrap();
    assert_eq!(result, 2);
}

#[test]
fn setmcnst_nested_path_test() {
    let code = r#"
    module X
      module Y
      end
    end
    X::Y::Z = 9
    def setmcnst_nested_path
      X::Y::Z
    end
    "#;
    let binary = mrbc_compile("setmcnst_nested_path", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "setmcnst_nested_path", &args).unwrap();
    let result: i64 = result.try_into().unwrap();
    assert_eq!(result, 9);
}

#[test]
fn setmcnst_class_const_test() {
    let code = r#"
    class C
    end
    C::K = 5
    def setmcnst_class_const
      C::K
    end
    "#;
    let binary = mrbc_compile("setmcnst_class_const", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "setmcnst_class_const", &args).unwrap();
    let result: i64 = result.try_into().unwrap();
    assert_eq!(result, 5);
}

#[test]
fn nested_define_module_preserves_existing_methods() {
    use mrubyedge::yamrb::helpers::mrb_define_module_cmethod;
    use mrubyedge::yamrb::value::RObject;
    use std::rc::Rc;

    let mut vm = mrubyedge::yamrb::vm::VM::empty();

    // First: define Outer module
    let outer = vm.define_module("Outer", None);

    // Define a cmethod on Outer
    mrb_define_module_cmethod(
        &mut vm,
        outer.clone(),
        "foo",
        Box::new(|_vm, _args| Ok(RObject::integer(42).to_refcount_assigned())),
    );

    // Define Inner nested under Outer
    let inner = vm.define_module("Inner", Some(outer.clone()));

    mrb_define_module_cmethod(
        &mut vm,
        inner.clone(),
        "bar",
        Box::new(|_vm, _args| Ok(RObject::integer(99).to_refcount_assigned())),
    );

    // Re-open Outer via define_module — should return the same module
    let outer2 = vm.define_module("Outer", None);
    assert!(
        Rc::ptr_eq(&outer, &outer2),
        "define_module should return the existing module"
    );

    // foo should still be defined on re-opened Outer
    assert!(
        outer2.procs.borrow().contains_key("foo"),
        "existing cmethod 'foo' should be preserved after re-opening"
    );

    // Re-open Inner nested under Outer — should return the same module
    let inner2 = vm.define_module("Inner", Some(outer.clone()));
    assert!(
        Rc::ptr_eq(&inner, &inner2),
        "nested define_module should return the existing module"
    );

    // bar should still be defined on re-opened Inner
    assert!(
        inner2.procs.borrow().contains_key("bar"),
        "existing cmethod 'bar' should be preserved after re-opening nested module"
    );
}

#[test]
fn include_nested_module_and_call_method() {
    use mrubyedge::yamrb::helpers::mrb_define_module_cmethod;
    use mrubyedge::yamrb::value::RObject;

    let mut vm = mrubyedge::yamrb::vm::VM::empty();
    let outer = vm.define_module("Outer", None);
    mrb_define_module_cmethod(
        &mut vm,
        outer.clone(),
        "foo",
        Box::new(|_vm, _args| Ok(RObject::integer(42).to_refcount_assigned())),
    );
    let inner = vm.define_module("Inner", Some(outer.clone()));
    mrb_define_module_cmethod(
        &mut vm,
        inner.clone(),
        "greet",
        Box::new(|_vm, _args| {
            Ok(RObject::string("hello from Inner".to_string()).to_refcount_assigned())
        }),
    );

    let script = r#"
class MyClass
  include Outer::Inner
end

MyClass.new.greet
"#;
    let binary = mrbc_compile("include_nested_module", script);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let result = vm.eval_rite(&mut rite).unwrap();

    let value: String = result
        .as_ref()
        .try_into()
        .expect("greet should return string");
    assert_eq!(value, "hello from Inner");
}
