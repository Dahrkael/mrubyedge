extern crate mec_mrbc_sys;
extern crate mrubyedge;

mod helpers;
use helpers::*;

// Regression: op_ssend places the self receiver at the result register
// (recv_index 0, result at R[a] with a != 0). do_op_send used to write the
// receiver unconditionally; the optimization must still do so for super
// sends, or the callee reads a stale self.

#[test]
fn self_send_with_value_result() {
    let code = "
def test_self_send
  foo + 1
end

def foo
  10
end
    ";
    let binary = mrbc_compile("self_send_value", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "test_self_send", &args).unwrap();
    let n: i64 = result.try_into().unwrap();
    assert_eq!(n, 11);
}

#[test]
fn self_send_inside_class_body() {
    // `include` is a bare private self-send in a class body; the receiver
    // must be the class object itself.
    let code = "
class MyCollection
  def each(&block)
    block.call(1)
    block.call(2)
  end
  include Enumerable
end

def test_collection_map
  MyCollection.new.map { |x| x * 2 }
end
    ";
    let binary = mrbc_compile("self_send_class_body", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    let args = vec![];
    let result = mrb_funcall(&mut vm, None, "test_collection_map", &args).unwrap();
    let result: (i32, i32) = result.try_into().unwrap();
    assert_eq!(result, (2, 4));
}
