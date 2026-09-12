extern crate mec_mrbc_sys;
extern crate mrubyedge;

mod helpers;
use helpers::*;

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
    let args = vec![int(1), int(2)];
    let result: bool = mrb_funcall(&mut vm, None, "check_eq_1", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert!(result);

    let args = vec![string("foo"), string("bar")];
    let result: bool = mrb_funcall(&mut vm, None, "check_eq_2", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert!(result);

    let args = vec![symbol("foo"), symbol("bar")];
    let result: bool = mrb_funcall(&mut vm, None, "check_eq_3", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert!(result);

    let args = vec![symbol("foo"), int(1), symbol("bar"), string("str")];
    let result: bool = mrb_funcall(&mut vm, None, "check_eq_4", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert!(result);
}
