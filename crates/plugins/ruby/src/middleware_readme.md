
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

## Retries and cancellation

Set `max_attempts: 3` on the client to enable bounded retries; the default is one attempt. `retry_base_delay: 0.5` and `retry_max_delay: 30` configure exponential backoff in seconds (maximum delay 60 seconds; maximum attempts 10). `retry-after-ms` and `Retry-After` override the delay within that cap. Transient transport failures and HTTP 408/429/500/502/503/504 are eligible. GET/HEAD/OPTIONS/PUT/DELETE can replay; POST/PATCH require a nonblank idempotency key. Generated custom key headers qualify only on their annotated operation. Automatic keys are created once by the operation and retained across attempts.

Middleware runs for each attempt. Pass `cancelled: -> { cancellation_requested }` to check cancellation before execution and during backoff; cancellation raises `KajiCancellationError`. An already executing synchronous transport needs its own interruption mechanism. Decode failures and ordinary client errors are not retried.
