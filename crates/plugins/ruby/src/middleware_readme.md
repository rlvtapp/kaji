
## Customer middleware

The first configured callable is outermost:

```ruby
add_header = lambda do |request, following|
  request['X-Customer'] = 'acme'
  following.call(request)
end
client = __MODULE__::Client.new(
  base_url: 'https://api.example.com',
  middleware: [add_header]
)
```

Callables receive native Net::HTTP requests and may rewrite requests/responses, recover errors, or return a response without calling `following`. A custom `transport:` callable replaces Net::HTTP execution and must return an object exposing native response `code` and `body` methods. Keep shared middleware state safe for concurrent calls and avoid logging authentication headers. This runtime currently has no automatic retries or SSE streaming.

Middleware supplied by the SDK author during generation is bundled as readable source and registered automatically. Instantiate the client normally to use those defaults. Constructor middleware adds wrappers after the bundled defaults.
