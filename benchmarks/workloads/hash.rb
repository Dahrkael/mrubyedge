# expected: 499500
def benchmark
  hash = {}
  i = 0
  while i < 1000
    hash["key_#{i}"] = i
    i += 1
  end
  sum = 0
  i = 0
  while i < 1000
    sum += hash["key_#{i}"]
    i += 1
  end
  sum
end
