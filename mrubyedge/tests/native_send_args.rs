extern crate mrubyedge;

mod helpers;
use helpers::*;

#[test]
fn native_method_receives_args_and_block_test() {
    let code = r#"
    a = Array.new(3) { |i| i * 2 }
    b = [].push(1, 2, 3)
    [a, b]
    "#;
    let binary = mrbc_compile("native_send_args", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let outer: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        result.as_ref().try_into().unwrap();
    let a_arr: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        outer[0].as_ref().try_into().unwrap();
    let b_arr: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        outer[1].as_ref().try_into().unwrap();
    let a: Vec<i64> = a_arr
        .iter()
        .map(|r| r.as_ref().try_into().unwrap())
        .collect();
    let b: Vec<i64> = b_arr
        .iter()
        .map(|r| r.as_ref().try_into().unwrap())
        .collect();
    assert_eq!(a, vec![0, 2, 4]);
    assert_eq!(b, vec![1, 2, 3]);
}

#[test]
fn method_missing_forwards_name_and_args_to_native_test() {
    let code = r#"
    class Proxy
      def method_missing(name, *args)
        [name, args]
      end
    end

    Proxy.new.undefined_method(7, 8)
    "#;
    let binary = mrbc_compile("mm_forward", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let outer: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        result.as_ref().try_into().unwrap();
    let name: String = outer[0].as_ref().try_into().unwrap();
    let args_arr: Vec<std::rc::Rc<mrubyedge::yamrb::value::RObject>> =
        outer[1].as_ref().try_into().unwrap();
    let args: Vec<i64> = args_arr
        .iter()
        .map(|r| r.as_ref().try_into().unwrap())
        .collect();
    assert_eq!(name, "undefined_method");
    assert_eq!(args, vec![7, 8]);
}

#[test]
fn ruby_method_receives_args_and_block_test() {
    let code = r#"
    def m(a, b)
      yield(a + b)
    end
    m(3, 4) { |s| s * 10 }
    "#;
    let binary = mrbc_compile("ruby_send_args", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let v: i64 = result.as_ref().try_into().unwrap();
    assert_eq!(v, 70);
}
