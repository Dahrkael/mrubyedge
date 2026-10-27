# expected: 10000
class Counter
  def initialize
    @count = 0
  end
  def increment
    @count += 1
  end
  def count
    @count
  end
end

def benchmark
  counter = Counter.new
  i = 0
  while i < 10000
    counter.increment
    i += 1
  end
  counter.count
end
