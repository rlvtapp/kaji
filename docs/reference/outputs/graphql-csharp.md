# GraphQL → C# clients

Unreleased. .NET 8 packages use HttpClient, System.Text.Json and CancellationToken.
The deprecated `dotnet` target aliases the same C# generator.

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language csharp --output generated
```

Use npm `pluginCSharp({ contracts: { graphql: { style: 'flat' } } })`
or Rust `csharp::graphql(Some(input.handle())).flat()` inside a C# package.

## Call styles

Raw exposes static `GraphqlOperations.ReadUserAsync(client, variables, cancellationToken)`.
Flat exposes `client.ReadUserAsync(variables, cancellationToken)`.
Grouped exposes `client.Query.ReadUserAsync(variables, cancellationToken)`.
Custom `groups: { users: { read: 'ReadUser' } }` exposes `client.Users.ReadAsync(...)`.
Operations without variables omit the variables argument.

## Models and failures

Selection-specific records and `Optional<T>` preserve absent versus explicit null
input values. Responses contain `Data`, `Errors` and `Extensions`, retaining partial
data. `EnsureSuccess()` throws on GraphQL errors; transport failures throw independently.
Cancellation propagates to HTTP requests.

Required field presence is checked during JSON decoding; nonnull field values are
not revalidated at runtime. Custom scalars use `JsonElement`, enums retain strings.
Abstract result variants require a selected nonnull `__typename` discriminator
(including aliases); untagged abstract selections and dynamic fields remain unsupported. Opt-in
subscriptions use distinct-connection graphql-sse; incremental inputs use experimental
multipart deferSpec=20220824. `ScalarCodecs` callbacks transform `JsonNode` values;
generated custom scalar fields retain `JsonElement`.

All four styles compile with warnings treated as errors and execute against pinned
GraphQL.js 16.14.2, including mutation, partial errors, cancellation and malformed responses.
See the [support matrix](../../plugin-support-matrix.md).

## Advanced capabilities

See [subscriptions, scalar callbacks and incremental delivery](graphql-capabilities.md)
for opt-in configuration, native stream lifetime and the tested protocol boundary.
These additions are unreleased; historical checks below predate them.

## Generated source layout

Runtime, client, operation, model and group sources have separate folders. Partial declarations preserve existing public model and client paths; wide records can split at property boundaries. The `dotnet` alias uses this same layout.

The layout targets source files below **128 KiB**, grouping declarations and export
parts at semantic boundaries. An indivisible model or operation that exceeds this
budget is retained and listed in `.poolster/source-layout-diagnostics.json`;
this is a size diagnostic, not a claim that every possible schema produces small files.
Regeneration removes obsolete unchanged owned files and preserves unrelated user files.
Source customizations referring to old monolithic paths must be retargeted.
