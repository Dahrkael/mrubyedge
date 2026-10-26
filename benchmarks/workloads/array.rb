# expected: 499500
def benchmark
  array = []
  1000.times do |i|
    array << i
  end
  sum = 0
  array.each do |value|
    sum += value
  end
  sum
end
