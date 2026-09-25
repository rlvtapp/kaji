# Kaji Go plugin

`kaji-plugin-go` renders a native Go API package from Kaji's neutral
`kaji_core::Api` model. It emits Go models, a configurable HTTP client,
typed operation request types, and a `go.mod` without requiring a JavaScript
runtime.

```rust
let tree = kaji_plugin_go::generate_go_sdk(&api, "sdks/go", Some("email"))?;
tree.write_to("generated")?;
```

For a product-style resource facade, select it explicitly while preserving
the direct generated operations for migration:

```rust
use kaji_core::SdkClientStyle;

let tree = kaji_plugin_go::generate_go_sdk_with_style(
    &api,
    "sdks/go",
    Some("email"),
    SdkClientStyle::Namespaced,
)?;
```

This initializes resource fields such as `client.Contacts.Get(ctx, input)`.
The default `SdkClientStyle::Flat` remains `client.GetContact(ctx, input)`.

The generated package has no third-party dependencies. Configure it with
`ClientConfig { BaseURL, APIKey, APIKeyHeader, APIKeyPrefix, HTTPClient, Retry }` and
call typed operation methods with a `context.Context`.

Generated clients retry transient transport failures and `408`, `429`, and
`5xx` responses with exponential backoff. Retries are safe by default: `GET`,
`PUT`, `PATCH`, and `DELETE` retry, while a `POST` retries only when its
generated request includes an `Idempotency-Key` header. The default is three
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
