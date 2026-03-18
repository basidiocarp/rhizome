module Networking
  class HttpClient
    MAX_RETRIES = 3

    def initialize(base_url)
      @base_url = base_url
    end

    def get(path)
      request(:get, path)
    end

    def post(path, body)
      request(:post, path, body)
    end

    def self.default_client
      new("https://api.example.com")
    end

    private

    def request(method, path, body = nil)
      # implementation
    end
  end

  class Response
    attr_reader :status, :body

    def initialize(status, body)
      @status = status
      @body = body
    end

    def success?
      (200..299).include?(status)
    end
  end
end
