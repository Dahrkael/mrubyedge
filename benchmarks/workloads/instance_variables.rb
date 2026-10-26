# expected: 136006
class Store
  attr_accessor :a, :b, :c, :d, :e, :f, :g, :h
  def initialize
    @a = 1
    @b = 2
    @c = 3
    @d = 4
    @e = 5
    @f = 6
    @g = 7
    @h = 8
  end
  def update
    @a = @b + @c
    @d = @e + @f
    @g = @h + @a
    @d + @g + @a
  end
end

def benchmark
  store = Store.new
  sum = 0
  i = 0
  while i < 2000
    sum += store.update
    store.h = store.a
    sum += store.h + store.b + store.c + store.d + store.e + store.f + store.g
    i += 1
  end
  sum
end
