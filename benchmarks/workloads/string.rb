# expected: 10000
def benchmark
  sum = 0
  i = 0
  while i < 2000
    string = "hello" + "world"
    sum += string[0, 5].length
    i += 1
  end
  sum
end
