# expected: 90557031
# Dispatch 300 in-memory requests, covering success, authorization and errors.
class Response
  attr_reader :status, :body, :headers

  def initialize(status, body)
    @status = status
    @body = body
    @headers = { "Content-Type" => "text/plain" }
  end
end

class Controller
  def index(params)
    Response.new(200, "Products page #{params["page"]}")
  end

  def create(params)
    Response.new(201, "Order created: #{params["quantity"]}")
  end

  def account(params)
    Response.new(200, "Account #{params["user_id"]}")
  end
end

class Router
  def initialize
    @routes = { "/products" => { "GET" => "index" },
                "/orders" => { "POST" => "create" },
                "/account" => { "GET" => "account" } }
    @controller = Controller.new
  end

  def dispatch(request)
    methods = @routes[request["path"]]
    return Response.new(404, "Not found") unless methods
    action = methods[request["method"]]
    return Response.new(405, "Method not allowed") unless action
    if action != "index"
      return Response.new(401, "Unauthorized") unless request["authenticated"]
    end
    params = request["params"]
    if action == "index"
      @controller.index(params)
    elsif action == "create"
      @controller.create(params)
    else
      @controller.account(params)
    end
  end
end

def benchmark
  requests = []
  paths = ["/products", "/orders", "/account", "/account", "/missing", "/products"]
  methods = ["GET", "POST", "GET", "GET", "GET", "DELETE"]
  i = 0
  while i < 300
    kind = i % 6
    requests << { "path" => paths[kind], "method" => methods[kind],
                  "authenticated" => kind != 3,
                  "params" => { "page" => i % 10 + 1, "quantity" => i % 3 + 1,
                                "user_id" => i + 1 } }
    i += 1
  end
  router = Router.new
  checksum = 0
  i = 0
  while i < requests.length
    request = requests[i]
    response = router.dispatch(request)
    checksum += response.status * 1000 + response.body.length
    checksum += response.headers["Content-Type"].length
    i += 1
  end
  checksum
end
