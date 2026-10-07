
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

Enable structural checks on buffered successful JSON responses with
`Client.new(base_url: "https://api.example.com", validate_responses: true)`.
Checks run after the outer middleware returns, including cache responses, and
before model conversion. They cover required response fields (excluding
write-only fields), primitives, nullability, nested arrays/objects, references,
and supported union/intersection shapes. New enum strings and extra fields are
retained. `ResponseDecodeError` reports a structural path without payload values.
Declared JSON decoding is bounded to 10 MiB and nesting depth 128. Validation is
opt-in and does not add retries. Error bodies, streams and undeclared/non-JSON
responses are outside this check; this is not complete JSON Schema constraint
validation or strict `oneOf` exclusivity.
