# GraphQL → Swift clients

Unreleased. Swift 5.9+ packages use Foundation URLSession, async functions and
selection-specific Codable models. Package platforms are macOS 12+ / iOS 15+.

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language swift --output generated
```

Use npm `pluginSwift({ contracts: { graphql: { style: 'flat' } } })` or
Rust `swift::graphql(Some(input.handle())).flat()` in a Swift package.

## Styles and models

Flat calls `try await client.readUser(variables: variables)`.
Grouped calls `try await client.query.readUser(variables: variables)`;
custom `groups: { users: { read: 'ReadUser' } }` exposes `client.users.read(...)`.
Raw exposes free operation functions. Operations with no variables omit the argument.

Codable models contain only selected fields. `GraphqlField<T>` distinguishes
`.omitted`, `.null` and `.value(value)` for optional fields. Nonnullable fields
reject null during encoding/decoding; recursive input objects use reference models.
Custom scalars use `GraphqlJSON`, enums retain strings.

`GraphqlResponse` keeps data, errors and extensions together, including partial data.
`ensureSuccess()` throws `GraphqlFailure` for GraphQL errors; HTTP failures throw
`GraphqlHTTPError`. Client configuration supplies endpoint, session and headers.
Task cancellation propagates through URLSession.

Abstract result variants require a selected nonnull `__typename` discriminator
(including aliases); untagged abstract selections and dynamic fields remain unsupported. Opt-in subscriptions
use distinct-connection graphql-sse; incremental inputs use experimental multipart
deferSpec=20220824. `scalarCodecs` callbacks transform `GraphqlJSON` values without
changing generated fields to arbitrary domain types. Generated code compiles with warnings treated as errors and runs
against pinned GraphQL.js 16.14.2. See the [support matrix](../../plugin-support-matrix.md).

## Advanced capabilities

See [subscriptions, scalar callbacks and incremental delivery](graphql-capabilities.md)
for opt-in configuration, native stream lifetime and the tested protocol boundary.
These additions are unreleased; historical checks below predate them.

## Generated source layout

`Sources/<module>/` separates `Runtime/`, `Client/`, `Operations/`, `Models/` and `Groups/`. Swift Package Manager discovers every source. Public symbols and call paths are preserved; filenames include category/group context to avoid basename collisions.

The layout targets source files below **128 KiB**, grouping declarations and export
parts at semantic boundaries. An indivisible model or operation that exceeds this
budget is retained and listed in `.poolster/source-layout-diagnostics.json`;
this is a size diagnostic, not a claim that every possible schema produces small files.
Regeneration removes obsolete unchanged owned files and preserves unrelated user files.
Source customizations referring to old monolithic paths must be retargeted.
