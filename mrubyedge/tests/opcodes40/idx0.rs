use crate::helpers::*;

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
    let result: i64 = (&result).try_into().unwrap();

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
    let result: i64 = (&result).try_into().unwrap();

    // Assert
    assert_eq!(result, 100);
}
