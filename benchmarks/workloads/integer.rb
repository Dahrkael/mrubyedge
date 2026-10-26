# expected: 49995000
def benchmark
  i = 0
  sum = 0
  while i < 10000
    sum += i
    i += 1
  end
  sum
end
