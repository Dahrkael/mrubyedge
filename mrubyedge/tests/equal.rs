extern crate mrubyedge;

mod helpers;
use helpers::*;
use mrubyedge::yamrb::value::RObject;

#[test]
fn equal_test() {
    let code = "
    def check_eq_1(a, b)
      3 == a + b
    end

    def check_eq_2(a, b)
      \"foobar\" == a + b
    end

    def check_eq_3(a, b)
      [:foo, :bar] == [a, b]
    end

    def check_eq_4(a, b, c, d)
      ha = {}
      ha[a] = b
      ha[c] = d

      {foo: 1, bar: \"str\"} == ha
    end
    ";
    let binary = mrbc_compile("eq", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    // Assert
    let args = vec![
        RObject::integer(1).to_refcount_assigned(),
        RObject::integer(2).to_refcount_assigned(),
    ];
    let result: bool = mrb_funcall(&mut vm, None, "check_eq_1", &args)
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert!(result);

    let args = vec![
        RObject::string("foo".into()).to_refcount_assigned(),
        RObject::string("bar".into()).to_refcount_assigned(),
    ];
    let result: bool = mrb_funcall(&mut vm, None, "check_eq_2", &args)
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert!(result);

    let args = vec![
        RObject::symbol("foo".into()).to_refcount_assigned(),
        RObject::symbol("bar".into()).to_refcount_assigned(),
    ];
    let result: bool = mrb_funcall(&mut vm, None, "check_eq_3", &args)
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert!(result);

    let args = vec![
        RObject::symbol("foo".into()).to_refcount_assigned(),
        RObject::integer(1).to_refcount_assigned(),
        RObject::symbol("bar".into()).to_refcount_assigned(),
        RObject::string("str".into()).to_refcount_assigned(),
    ];
    let result: bool = mrb_funcall(&mut vm, None, "check_eq_4", &args)
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert!(result);
}

#[test]
fn user_defined_eq_is_honored_test() {
    let code = r#"
    class Money
      def initialize(amount)
        @amount = amount
      end

      def ==(other)
        @amount == other.amount
      end

      def amount
        @amount
      end
    end

    [Money.new(5) == Money.new(5), Money.new(5) == Money.new(6), 1 == 1, "a" == "a"]
    "#;
    let binary = mrbc_compile("user_defined_eq", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let arr: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        result.as_ref().try_into().unwrap();
    let vals: Vec<bool> = arr.iter().map(|r| r.as_ref().try_into().unwrap()).collect();
    assert_eq!(vals, vec![true, false, true, true]);
}
