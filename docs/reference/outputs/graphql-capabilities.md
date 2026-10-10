# Advanced GraphQL capabilities

Unreleased alpha.2 work. All ten SDK outputs provide opt-in subscriptions, runtime
scalar callbacks and experimental incremental delivery. The CLI and Postman
**output generators** still generate fixed query/mutation tools; these SDK
capabilities do not extend those tools or the query-hook integrations.

## Supply a schema

The GraphQL input accepts SDL or a local introspection JSON document, with either
`__schema` at the root or a GraphQL response containing `data.__schema`. It does
not fetch an introspection endpoint. Named operation documents remain required
for usable clients.

SDL and operation files can include quoted full-file imports:

```graphql
# import "./shared.graphql"
# import * from "types.graphql"
```

Imports resolve beside the importing file and then through configured import
roots. Cycles, unresolved imports and malformed introspection fail before
publication. Selective named imports are unsupported.

```js
input: {
  path: './schema.json', plugin: inputGraphql(),
  operations: ['./operations.graphql'], importRoots: ['./schemas'],
}
```

The JSON recipe equivalent uses `input.options.operation_files` and
`input.options.import_roots`. CLI generation accepts repeated `--operation` and
`--import-root` options.

## Subscriptions

Enable `subscriptions: true` in `contracts.graphql`. Rust factories use
`.subscriptions()`; Java/C#/Swift factories use `.subscriptions(true)`.
The bundled transport uses **distinct-connection graphql-sse**: each operation
POSTs its document and variables, with `Accept: text/event-stream`, and receives
`next` envelopes followed by `complete`. Headers carry the same authentication as
HTTP queries. Results retain GraphQL errors and partial data.

TypeScript also retains its injected `SubscriptionTransport` interface:

```ts
import { createClient, createGraphqlSseTransport } from './generated/index.js';

const subscriptionTransport = createGraphqlSseTransport(endpoint, {
  headers: { Authorization: `Bearer ${token}` },
});
const client = createClient({ endpoint, subscriptionTransport });
for await (const result of client.changed({ id: '42' })) {
  console.log(result);
}
```

Other languages use their native stream/iterator APIs. Close or cancel streams
when finished: Java streams must be closed, Rust streams can be dropped, and
Python/PHP generators must be closed or released. Cancellation semantics and
buffer limits follow the target runtime. There is no bundled WebSocket transport,
single-connection multiplexing, reconnection or replay.

## Runtime scalar callbacks

Encoding and decoding are separate callbacks. The generated transport walks
selected field shapes, nested lists and named input objects. Missing fields and
explicit null bypass scalar callbacks. Abstract selections need an unambiguous
selected discriminator when choosing a codec-bearing variant. Default behavior
remains unchanged when no callbacks are supplied.

TypeScript maps application types independently in `contracts.graphql.scalars`;
the caller supplies conversion functions at runtime:

```ts
// Generation: scalars: { DateTime: { input: 'Date', output: 'Date' } }
const client = createClient({ endpoint, scalarCodecs: {
  DateTime: {
    encode: value => value.toISOString(),
    decode: value => new Date(String(value)),
  },
} });
```

Raw TypeScript operations accept codecs through their transport or request
options. Rust keeps its existing validated self-contained mapping types and adds
`GraphqlScalarCodecs` on the transport; the mapping API does not supply arbitrary
external types or dependencies.

| Target | Callback configuration | Value boundary |
| --- | --- | --- |
| TypeScript | `scalarCodecs` with `encode` / `decode` | Configured input/output application types |
| Rust | Transport `.with_codecs(GraphqlScalarCodecs::new().with::<I,O>(...))` | Supported mapped types ↔ JSON values |
| Go | Client `WithScalarCodecs` with `ScalarCodec` / `TypedScalarCodec` | Supported mapped types ↔ JSON wire data |
| Python | Client `scalar_codecs` dictionary | Custom scalars remain statically `Any` |
| PHP | Client `scalarCodecs` array | Custom scalar model values remain `mixed` |
| Ruby | Client `scalar_codecs` hash | Custom scalar signatures remain untyped |
| Java | `HttpTransport` codec map | `JsonNode` ↔ `JsonNode` |
| C# | Client `ScalarCodecs` property | `JsonNode` ↔ `JsonNode`; models retain `JsonElement` |
| Swift | Client `scalarCodecs` dictionary | `GraphqlJSON` ↔ `GraphqlJSON` |
| Elixir | Client `scalar_codecs` option | Elixir terms; model validation still applies |

A mapping alone does not convert a date string. Codec failures are transport or
conversion failures, distinct from server GraphQL errors. JSON-domain callbacks
in Java/C#/Swift do not add arbitrary domain object types to generated models.

## Experimental defer / stream

Set `input.incremental: true` in the npm configuration, or
`input.options.graphql_incremental: true` in a JSON recipe. Direct CLI generation
uses `--graphql-incremental` (add `--raw-sdk` for TypeScript). The input publishes a
separate `GraphqlIncrementalOperations` contract; SDK factories expose
`graphql_incremental(Some(input_handle))`. Ordinary operation lowering continues
to reject `@defer` / `@stream` unless this capability is selected.

The tested protocol is **multipart/mixed; deferSpec=20220824**, using path-based
`incremental` patches and boolean `hasNext`. This is an explicit experimental
variant, not support for every evolving GraphQL incremental protocol. Newer
`pending` / `completed` ID-based payloads are rejected. Abstract selections must
select a common, unconditional `__typename` outside deferred fragments (aliases
are supported); unsafe deferred discriminators are rejected during input lowering. Custom executable
directives and dynamic caller-selected fields remain unsupported.

Partial snapshots retain incomplete data and patches; they are not deserialized
into the final selection type early. Language APIs differ:

| Target | Incremental result |
| --- | --- |
| TypeScript | Raw incremental helper and accumulated snapshots; no generated client facade |
| Rust | `GraphqlIncrementalEvent`: snapshot or final `GraphqlResponse<T>` |
| Go | Incremental iterator with partial snapshots and final selected result |
| Python / PHP / Ruby | Frames with raw partial data and an optional final typed envelope |
| Java / C# / Swift | Snapshots with a completion flag; final typed decoding requires completion and no GraphQL errors |
| Elixir | Partial map snapshots; `require_data` decodes the final selection only after completion without GraphQL errors |

TypeScript incremental generation is **raw only**. Other SDK generators retain
raw, flat and grouped styles, subject to configuration-layer compatibility checks.
Do not apply query-hook or mock/validator integrations to incremental contracts.
Network, malformed framing and unsupported protocol variants fail explicitly.

## Verification boundary

Generated packages are compiled and executed against pinned local GraphQL and
subscription servers; multipart fixtures test the explicit dialect above.
Language guides retain their model and abstract-type limitations. Historical
verification records describe the checkout they tested, not automatic support
for these later additions. See the [support matrix](../../plugin-support-matrix.md)
and the [batch verification record](../../verification-results/graphql-advanced-2026-10-10.json).
