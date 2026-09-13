use crate::helpers::*;

const CHUNK: &str = "
class Counter
  def initialize(start)
    @n = start
  end

  def bump(by)
    @n = @n + by
    self
  end

  def value
    @n
  end

  def empty
  end

  def yes
    true
  end

  def no
    false
  end

  def self.build(start)
    new(start)
  end
end

def locals
  a = 1
  b = 2
  c = 3
  a += 4
  b -= 1
  [a, b, c].join(\"-\")
end

def first_of(list)
  list[0]
end

def through_block
  yield 3
end

def bare
  7
end

counter = Counter.build(10)
counter.bump(5)
counter.bump(-3)

parts = [
  counter.value,
  locals,
  first_of([1, 2, 3]),
  through_block { |x| x * 2 },
  bare,
  counter.empty.nil? ? 1 : 0,
  counter.yes ? 1 : 0,
  counter.no ? 0 : 1
]
parts.join(\",\")
";

#[test]
fn a_whole_mruby_40_chunk_runs_test() {
    let binary = mrbc_compile("compiled", CHUNK);
    let mut rite = mrubyedge::rite::load(&binary).unwrap();
    let mut vm = mrubyedge::yamrb::vm::VM::open(&mut rite);
    let result = vm.run().unwrap();
    let result: String = result.as_ref().try_into().unwrap();

    // Assert
    assert_eq!(result, "12,5-1-3,1,6,7,1,1,1");
}
