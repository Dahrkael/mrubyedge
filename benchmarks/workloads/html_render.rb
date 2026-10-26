# expected: 231128477
# Build 100 article records and render 75 published articles with HTML escaping.
class ArticleRenderer
  def initialize
    @escapes = { "&" => "&amp;", "<" => "&lt;",
                 ">" => "&gt;", '"' => "&quot;" }
  end

  def escape(text)
    output = ""
    i = 0
    while i < text.length
      char = text[i, 1]
      replacement = @escapes[char]
      if replacement
        output << replacement
      else
        output << char
      end
      i += 1
    end
    output
  end

  def render(articles)
    html = "<ul>"
    i = 0
    while i < articles.length
      article = articles[i]
      if article["published"]
        html << '<li><a href="/articles/'
        html << article["id"].to_s
        html << '">'
        html << escape(article["title"])
        html << '</a><span class="author">'
        html << escape(article["author"])
        html << '</span></li>'
      end
      i += 1
    end
    html << "</ul>"
    html
  end
end

def benchmark
  articles = []
  i = 0
  while i < 100
    articles << { "id" => i, "title" => "Ruby <edge> & tips #{i}",
                  "author" => 'Writer "A"', "published" => i % 4 != 0 }
    i += 1
  end
  renderer = ArticleRenderer.new
  raise "incorrect escaping" unless renderer.escape('<&">') == "&lt;&amp;&quot;&gt;"
  html = renderer.render(articles)
  raise "incorrect document" unless html[0, 4] == "<ul>" && html[html.length - 5, 5] == "</ul>"
  # A content checksum validates the complete output, not only its length.
  checksum = 0
  i = 0
  while i < html.length
    checksum = (checksum * 31 + html.getbyte(i)) % 1000000007
    i += 1
  end
  checksum
end
