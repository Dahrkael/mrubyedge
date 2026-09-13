use crate::helpers::*;

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
