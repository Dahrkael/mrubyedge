extern crate mec_mrbc_sys;
extern crate mrubyedge;

mod helpers;
use helpers::*;

#[test]
fn break_test() {
    let code = "
    def myyield
      yield 1
      yield 1
      yield 1
      yield 1
      yield 1
    end

    def test_break
      i = 0
      myyield do
        i += 1
        break i if i >= 3
      end
    end
    ";
    let binary = mrbc_compile("break_test", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    // Assert
    let args = vec![];
    let result: i32 = mrb_funcall(&mut vm, None, "test_break", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 3);
}

#[test]
fn break_test_with_c_func() {
    let code = "
    def test_break
      i = 0
      10.times do
        i += 1
        break i if i >= 5
      end
    end
    ";
    let binary = mrbc_compile("break_test_with_c_func", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    // Assert
    let args = vec![];
    let result: i32 = mrb_funcall(&mut vm, None, "test_break", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 5);
}

#[test]
fn break_test_with_c_func_2() {
    let code = "
    def test_break
      i = 0
      10.times do
        puts \"loop #{i}\"
        i += 1
        break i if i >= 5
      end
    end
    ";
    let binary = mrbc_compile("break_test_with_c_func_2", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    // Assert
    let args = vec![];
    let result: i32 = mrb_funcall(&mut vm, None, "test_break", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, 5);
}

#[test]
fn break_test_nested() {
    let code = "
    def myyield
      yield 1
      yield 1
      yield 1
      yield 1
      yield 1
    end

    def myyield2
      yield 2
      yield 2
      yield 2
      yield 2
      yield 2
    end

    def test_break
      x = 0
      y = 0
      myyield do |i|
        y = 0
        myyield2 do |j|
          y += j
          break if y >= 6
        end
        puts \"loop #{x}, #{y}\"
        x += 1
      end
      [x, y]
    end
    ";
    let binary = mrbc_compile("break_test_with_c_func_nested", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    // Assert
    let args = vec![];
    let result: (i32, i32) = mrb_funcall(&mut vm, None, "test_break", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, (5, 6));
}

#[test]
fn break_test_nested_with_closure() {
    let code = "
    $a = [0, 2, 4, 6, 8]

    def test_break
      x = 0
      y = 0
      3.times do |i|
        $a.each do |j|
          y += j
          break if j >= 5
        end
        x += i
      end
      [x, y]
    end
    ";
    let binary = mrbc_compile("break_test_nested_with_closure", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();
    // Assert
    let args = vec![];
    let result: (i32, i32) = mrb_funcall(&mut vm, None, "test_break", &args)
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(result, (3, 36));
}

#[test]
fn break_test_toplevel() {
    let code = "
    i = 0
    v = 10.times do
      i += 2
      break i if i >= 10
    end
    v
    ";
    let binary = mrbc_compile("break_test_toplevel", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);

    // Assert
    let result: i32 = vm.run().unwrap().try_into().unwrap();
    assert_eq!(result, 10);
}

#[test]
fn dump_block_registers() {
    let code = "
    def render
      test = 2
      draws = []
      [1].each do |sprite|
        test = sprite
      end
      draws
    end
    render
    ";
    let binary = mrbc_compile("dump_block_registers", code);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    vm.run().unwrap();

    fn dump(name: &str, irep: &mrubyedge::yamrb::vm::IREP, depth: usize) {
        let pad = "  ".repeat(depth);
        let lv = irep
            .lv
            .as_ref()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{v}@reg{k}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "-".to_string());
        eprintln!(
            "[dump] {pad}{name}: nlocals={} nregs={} locals=[{lv}]",
            irep.nlocals, irep.nregs
        );
        for op in irep.code.iter().take(14) {
            eprintln!("[dump] {pad}  {:?} {:?}", op.code, op.operand);
        }
        for (i, rep) in irep.reps.iter().enumerate() {
            dump(&format!("block{i}"), rep, depth + 1);
        }
    }
    dump("main", &vm.irep, 0);
}
