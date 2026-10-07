# Kaji Ruby plugin

Generates a Ruby 3.1+ gem from Kaji's normalized API model. The resulting gem
uses Ruby's standard `net/http`, `uri`, and `json` libraries, so application
owners keep control of their HTTP stack and dependency policy.

Use `ruby::package("ruby").with(ruby::sdk())` in an embedded profile, or set
`"language": "ruby"` with the `sdk` plugin in `kaji.json`.

Generated object models retain explicitly supplied nulls separately from omitted
optional fields. Open models retain unknown keys at their original wire names;
the extra-properties map cannot overwrite a declared field during encoding.
The synthetic map accessor is renamed when an API declares a property with the
same name.

Add `.with(ruby::roundtrips())` to emit bounded wire fixtures and
`test/model_roundtrips.rb`. The consumer resolves the `RubyModels` provider
contract automatically; `.models_from(&sdk)` selects an explicit SDK. Run
`ruby -Ilib test/model_roundtrips.rb` in the generated gem. Fixture diagnostics
identify schemas that could not be sampled within configured bounds.

### Customer transport middleware

The client accepts `transport:` (a callable receiving a native `Net::HTTPRequest`) and `middleware:` (an array of callables receiving `request, next`). The default transport uses `Net::HTTP` with the existing timeout settings. The first middleware is outermost; native request headers/body can be changed, responses or errors replaced, and cached responses returned without calling `next`. Normal generated response decoding and `ApiError` handling run after the chain.

```ruby
add_header = lambda do |request, following|
  request['X-Customer'] = 'acme'
  following.call(request)
end
client = ExampleSdk::Client.new(
  base_url: 'https://api.example.com',
  middleware: [add_header]
)
```

A custom transport must return an object supporting the native response `code` and `body` methods. Middleware replacing the URI should pass a new native request to `next`; the default transport uses that request's URI. Middleware configuration is copied at construction. Shared mutable middleware state must support concurrent calls. This Ruby runtime does not currently implement automatic retries or SSE streaming.

### Bundle author middleware during generation

The package builder's `.middleware(BundledMiddleware { path, contents, symbol, async_symbol: None })` ships an author-owned `.rb` file beside `lib/<sdk>/client.rb`. Define `symbol` as a top-level callable constant accepting `(request, following)`. The generated client requires the source and prepends it to its default chain, so SDK consumers need no middleware setup. Bundled defaults run before optional customer wrappers in configuration order. Native source-path/symbol mismatches and generated-file collisions fail generation.
