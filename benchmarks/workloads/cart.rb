# expected: 211029
# One invocation builds and prices three carts (100, 10 and 2 line items).
class LineItem
  def initialize(price, quantity, category)
    @price = price
    @quantity = quantity
    @category = category
  end

  def subtotal
    @price * @quantity
  end

  def discountable?
    @category != "book"
  end
end

class Cart
  def initialize(member)
    @member = member
    @items = []
  end

  def add(item)
    @items << item
  end

  def invoice(coupon)
    subtotal = 0
    eligible = 0
    i = 0
    while i < @items.length
      item = @items[i]
      amount = item.subtotal
      subtotal += amount
      eligible += amount if item.discountable?
      i += 1
    end
    discount = 0
    discount = eligible * 10 / 100 if @member
    discount += 300 if coupon && subtotal >= 5000
    net = subtotal - discount
    shipping = 0
    shipping = 500 if net < 5000
    { "subtotal" => subtotal, "discount" => discount,
      "tax" => net * 10 / 100, "shipping" => shipping,
      "total" => net + net * 10 / 100 + shipping }
  end
end

def benchmark
  checksum = 0
  counts = [100, 10, 2]
  scenario = 0
  while scenario < counts.length
    count = counts[scenario]
    cart = Cart.new(count != 10)
    i = 0
    while i < count
      category = "general"
      category = "book" if i % 4 == 0
      cart.add(LineItem.new(100 + i * 17, i % 3 + 1, category))
      i += 1
    end
    invoice = cart.invoice(count == 100)
    checksum += invoice["total"] + invoice["discount"] + invoice["shipping"]
    scenario += 1
  end
  checksum
end
