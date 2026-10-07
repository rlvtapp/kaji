# Kaji Go plugin

`kaji-plugin-go` renders SDK packages from Kaji's neutral API model,
using a standard-library HTTP client. All generation runs in Rust.

```rust
use kaji::{go, prelude::*};

let release = ProfileSet::new("sdk")
    .package(go::package("go")
        .name("email")
        .with(go::sdk()));
let tree = kaji::generate(&api, release)?;
tree.write_to("generated")?;
```

SDKs are namespaced by default. Choose `go::sdk().flat()` or
`.namespaced()` explicitly, or supply a shared `Common` default.
Namespaced clients expose resources such as `client.Contacts.Get(ctx, input)`;
flat clients use direct operations such as `client.GetContact(ctx, input)`.
Go always emits split model and operation files. Configure bounded rendering
parallelism with `go::sdk().jobs(4)`; see [large specs](../../../docs/large-specs.md).

When depending on this plugin without the `kaji` facade, import
`kaji_plugin_go::PackageExt` and compose its package through
`kaji_core::engine::Packages`. Supply a security catalog when your API
declares named security schemes.

See [configuration](../../../docs/configuration.md) for every generation option
and the generated package's README for exact operation signatures.

The generated package has no third-party dependencies. Configure it with
`ClientConfig { BaseURL, APIKey, APIKeyHeader, APIKeyPrefix, HTTPClient, Retry }` and
call typed operation methods with a `context.Context`.

Generated clients retry transient transport failures and `408`, `429`, and
`5xx` responses with exponential backoff. Retries are safe by default: `GET`,
`PUT` and `DELETE` retry, while `POST` and `PATCH` retry only with a nonempty
`Idempotency-Key` or the declared custom idempotency header on an opted-in operation.
Auto-generated keys use secure UUID v4 randomness once per operation call and stay
stable across retries; explicit caller keys are preserved. Server `Retry-After-Ms`
and `Retry-After` delays respect the configured delay cap. The default is three
attempts (250ms initial delay, 8s cap). Set `Retry: &RetryConfig{MaxAttempts:
1}` to disable retries, or tune the delays without replacing the configured
`HTTPClient`.

## Declared errors and response media

Each declared non-success OpenAPI response becomes an operation-specific Go
error, such as `*GetContactError404`. It includes the HTTP status, untouched
`RawBody`, and a typed `Body` when the response declares a schema. Use normal
Go error matching:

```go
contact, err := client.GetContact(ctx, input)
var notFound *GetContactError404
if errors.As(err, &notFound) {
    fmt.Println(notFound.Body)
}
_ = contact
```

`application/octet-stream` and binary schemas return `[]byte`; `text/*`
responses return `string`; and `text/event-stream` returns an `io.ReadCloser`.
The caller closes an SSE stream. A stream is retried only before it is opened;
reconnection after events have started is application policy.

## Cursor pagination

An operation with `x-kaji-pagination` (or `x-speakeasy-pagination`) configured
as a cursor pager gets a typed `{Operation}Pages` constructor. It returns a
pager whose `Next(ctx)` method yields normal response pages and then `io.EOF`.
Kaji creates this surface for an optional string cursor parameter and a
declared `outputs.nextCursor` object JSONPath. It also supports a cursor in a
JSON request body when that body is a named object schema with an optional
string cursor field. The pager clones that typed body before setting the next
cursor, so the caller's request is never mutated.

For `type: url`, Kaji follows a declared `outputs.nextUrl` continuation only
through a private, same-origin request path. It reuses the operation's HTTP
method, generated header parameters, authentication, and JSON body encoding;
cross-origin URLs are rejected. There is no public raw-URL override API.

```go
pager := client.ListContactsPages(&ListContactsRequest{})
for {
    page, err := pager.Next(ctx)
    if errors.Is(err, io.EOF) { break }
    if err != nil { return err }
    use(page)
}
```

### Customer transport middleware

`ClientConfig.Middleware` accepts `[]KajiMiddleware`. Each entry wraps a `KajiHTTPClient`; `KajiHTTPClientFunc` adapts ordinary functions to its `Do` interface. The first configured middleware is outermost, and the chain executes for each HTTP attempt, including retries, pagination and SSE establishment. Existing `HTTPClient` injection and telemetry hooks remain available.

```go
middleware := func(next KajiHTTPClient) KajiHTTPClient {
    return KajiHTTPClientFunc(func(request *http.Request) (*http.Response, error) {
        rewritten := request.Clone(request.Context())
        rewritten.Header.Set("X-Customer", "acme")
        return next.Do(rewritten)
    })
}
client, err := NewClient(ClientConfig{
    BaseURL: "https://api.example.com",
    Middleware: []KajiMiddleware{middleware},
})
```

Middleware can rewrite requests/responses, recover or replace errors, or return a response without calling the wrapped client. Keep the request context for cancellation and follow `net/http` response-body ownership conventions: close discarded response bodies yourself; the SDK closes returned bodies after consuming them. Wrappers must support concurrent use. Nil middleware or a nil returned transport fails construction. Middleware sees native requests, including authentication headers.

### Bundle author middleware during generation

The package builder's `.middleware(BundledMiddleware { path, contents, symbol, async_symbol: None })` ships and registers an author-supplied native Go wrapper automatically. Use a `.go` file beside the generated client, declaring the same package, and a symbol with the `KajiMiddleware` function ABI. Consumers need no `ClientConfig.Middleware` registration. Bundled defaults run before optional customer wrappers in configuration order. Source filenames/build constraints must not restrict compilation to a platform. Colliding paths, foreign package declarations and transports lacking the native registration boundary fail generation.

Optional native operation smoke tests can be distributed with the generated SDK:

```rust
kaji::go::package("go")
    .with(kaji::go::sdk())
    .with(kaji::go::operation_tests().max_operations(128))
```

The consumer resolves the typed Client/Operations contracts and emits executable
`operation_generated_test.go`, a distributed `OPERATION_TESTS.md`, and
`.kaji/operation-test-diagnostics.json`. Run `go test -v ./...` to see asserted
wire cases and explicit skips. It uses bounded core samples and a fake native
transport; supported public operations exercise path/query/header serialization,
JSON request bodies, JSON response decoding, and void success statuses. It does
not contact the service. Custom ABIs, other media kinds, unsupported schemas and
operation bounds are reported rather than advertised as tested. Explicit
`using_client`/`using_operations` handles are available for composed profiles.
