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
Abstract union selections, subscriptions, incremental delivery and dynamic selections
are unsupported in this first implementation.

All four styles compile with warnings treated as errors and execute against pinned
GraphQL.js 16.14.2, including mutation, partial errors, cancellation and malformed responses.
See the [support matrix](../../plugin-support-matrix.md).
