# GraphQL → Go clients

Unreleased. Generate query/mutation clients from a schema plus operation documents.
The generated runtime uses Go's standard library and requires Go 1.22+.

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language go --output generated
```

The existing npm `pluginGo` can select the same pipeline with
`contracts: { graphql: { style: 'flat' } }`. Rust composition uses
`go::graphql(Some(input.handle())).flat()` in a Go package.

## Call an operation

For an operation named `ReadUser`, the generated API has this shape:

```go
client := graphqlclient.NewClient(endpoint, httpClient)
response, err := client.ReadUser(ctx, graphqlclient.ReadUserVariables{ID: "42"})
if response != nil && response.Data != nil {
    // Data may be usable even when err contains GraphQL application errors.
}
if err != nil {
    // Inspect GraphQLErrors separately from HTTP/JSON/transport failures.
}
```

Use your generated module/package and field names. Context cancellation propagates
to the HTTP request. Headers are configurable on the bound client; application
HTTP clients control timeouts and transport behavior.

`raw` exposes standalone operation functions. `flat` adds exported client methods.
`idiomatic` exposes `client.Query().ReadUser(ctx, variables)` and mutation groups.
Explicit `.group("users", "read", "ReadUser")` configuration exposes
`client.Users().Read(ctx, variables)`.

## Models and presence

Result structs contain only the operation's selections. Variables and input
objects preserve omission independently of explicit null through `Optional[T]`:
use its zero value for absence, `Some(value)` for a value and `Null[T]()` for null.
Required nullable fields use pointer values. Model shapes encode selection and
nullability, but there is no runtime schema validator: Go JSON decoding can turn
missing required primitive fields into zero values.

GraphQL application errors return both an envelope and `GraphQLErrors`, preserving
partial data. Non-2xx HTTP responses, invalid JSON, empty envelopes and network
errors are distinct failures. Responses are bounded to 16 MiB.

## Limits

Opt-in subscriptions use distinct-connection graphql-sse; incremental inputs use
experimental multipart deferSpec=20220824. Dynamic selections remain unsupported.
Interface/union result variants require a selected nonnull `__typename`
discriminator; aliases are retained. Untagged abstract selections are rejected.
Enums retain wire strings; custom scalars default to `json.RawMessage`. Supported
scalar mappings and `ScalarCodec` / `TypedScalarCodec` callbacks provide separate
input/output conversion; `WithScalarCodecs` configures the client. No third-party GraphQL runtime is required in generated packages.

Generated-code tests cover all styles, presence/null behavior, exact documents,
errors, cancellation and regeneration. The [support matrix](../../plugin-support-matrix.md)
records the validation boundary.

## Advanced capabilities

See [subscriptions, scalar callbacks and incremental delivery](graphql-capabilities.md)
for opt-in configuration, native stream lifetime and the tested protocol boundary.
These additions are unreleased; historical checks below predate them.

## Generated source layout

Go keeps one package and the existing import path. Separate `graphql_runtime.go`, `graphql_client.go`, `graphql_transport.go`, `graphql_model_*.go`, `graphql_operation_*.go` and group/method files replace `graphql.go`. Same-package Go source files intentionally share one directory.

The layout targets source files below **128 KiB**, grouping declarations and export
parts at semantic boundaries. An indivisible model or operation that exceeds this
budget is retained and listed in `.poolster/source-layout-diagnostics.json`;
this is a size diagnostic, not a claim that every possible schema produces small files.
Regeneration removes obsolete unchanged owned files and preserves unrelated user files.
Source customizations referring to old monolithic paths must be retargeted.
