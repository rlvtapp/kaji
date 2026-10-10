# GraphQL → TypeScript client generation

Poolster generates a TypeScript package from a complete GraphQL SDL schema and
operation documents. Queries and mutations have selection-specific result types,
variable types and functions. This is part of the TypeScript output package,
which also supports OpenAPI HTTP generation.

## Generate with the CLI

```sh
poolster generate schema.graphql --input-format graphql \
  --provider graphql.apollo --operation operations.graphql \
  --language typescript --output generated
```

Repeat `--operation` for multiple files. Operations are validated against the
schema before generation. Schema-only input can be inspected, but client generation
requires operations. Use a recipe for package metadata and custom scalar mappings:

```json
{
  "input": {
    "format": "graphql",
    "provider": "graphql.apollo",
    "path": "schema.graphql",
    "options": { "operation_files": ["operations.graphql"] }
  },
  "output": { "path": "generated" },
  "packages": [{
    "language": "typescript",
    "path": "client",
    "name": "@example/graphql-client",
    "version": "1.0.0",
    "plugins": [{
      "name": "graphql",
      "contracts": {
        "graphql": {
          "style": "flat",
          "scalars": { "DateTime": { "input": "string", "output": "string" } }
        }
      }
    }]
  }]
}
```

Paths resolve relative to the recipe. Run `poolster generate --config poolster.json`.
Build the emitted package using its pinned TypeScript dependency, then import its
compiled package exports. The [runnable example](../../../examples/graphql-native/README.md)
includes a local server and a consumer.

## Generate from Node

The existing TypeScript plugin package also generates GraphQL clients:

```js
import { generate } from '@relevate/poolster';
import { inputGraphql } from '@relevate/poolster-input-graphql';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';

await generate({
  input: {
    path: './schema.graphql',
    plugin: inputGraphql(),
    operations: ['./operations.graphql'],
  },
  output: './generated',
  plugins: [pluginTypeScript({
    path: 'client',
    name: '@example/graphql-client',
    contracts: {
      graphql: {
        style: 'flat',
        scalars: { DateTime: { input: 'string', output: 'string' } },
      },
    },
  })],
});
```

Config-file loading resolves operation paths alongside the config file.
Each output plugin configures its own scalar mappings under `contracts.graphql`.
The older `input.scalars` and `input.rustScalars` options remain shorthand for
TypeScript and Rust mappings respectively; conflicting mappings are rejected. This
entry point invokes the native GraphQL generator; it does not establish general
Node dispatch for Rust contract/block hooks.

## Scalar mappings

Each custom scalar mapping has separate `input` and `output` TypeScript types.
Input mappings apply to variables and input-object fields; output mappings apply
to selected results. GraphQL nullability, list element nullability and optional
presence remain separate from the scalar mapping. Unconfigured scalars stay
`unknown`. Mapping names must match custom scalars reachable from the generated
contract; unknown/unused names and overrides of built-in scalars are rejected.

Mappings describe application types; conversion requires caller-supplied runtime
`scalarCodecs` encode/decode callbacks. Without callbacks, a mapping alone does not
convert a JSON string into `Date`. Use wire-compatible mappings when omitting codecs.

Through the Rust API, configure the same generator:

```rust
let output = ts::graphql(Some(input.handle()))
    .scalar("DateTime", ts::GraphqlScalarMapping::new("string", "string"));
```

## Results and transport failures

```ts
import { createClient } from '@example/graphql-client';
const client = createClient({ endpoint: 'http://localhost:4000/graphql' });
// Generated from an operation named ReadUser.
const result = await client.readUser({ id: '42' });
switch (result.kind) {
  case 'success': console.log(result.data); break;
  case 'partial': console.log(result.data, result.errors); break;
  case 'error': console.error(result.errors); break;
}
```

GraphQL errors are returned explicitly, retaining partial data and extensions.
HTTP failures throw `GraphqlHttpError`; malformed response JSON or envelopes
throw `GraphqlProtocolError`. Fetch, headers and cancellation are configurable.
The HTTP transport does not retry mutations automatically.

## JavaScript client styles

The compiled package is usable directly from JavaScript. In flat style, named
operations become methods: `ReadUser` becomes `client.readUser(variables)` and
`RenameUser` becomes `client.renameUser(variables)`. Configure endpoint, fetch and
headers once through `createClient`; individual calls can provide request options.

Grouped style uses explicit operation mappings:

```js
pluginTypeScript({
  contracts: {
    graphql: {
      style: 'grouped',
      groups: {
        user: { read: 'ReadUser', rename: 'RenameUser' },
      },
    },
  },
});
// In the generated package:
await client.user.read({ id: '42' });
await client.user.rename({ id: '42', name: 'Ada' });
```

Unassigned operations use `query`, `mutation` or `subscription` groups. The
`namespaced` and `idiomatic` names are aliases for this grouped style; grouped is
the current default when no style is configured. Raw style emits standalone
functions without a bound client. Existing standalone operation exports remain
available in flat and grouped packages. Groups cannot be configured for flat or
raw style; unknown operations and method collisions are rejected.

Selections remain fixed by operation documents in every style. A typed dynamic
`fields` argument is only a [future proposal](../../proposals/graphql-selection-builder-proposal.md).

## Supported boundary

Query framework, validation and mock integrations are documented in the
[GraphQL integration guide](graphql-integrations.md); their checks are separate
from core client compilation and runtime tests.

Aliases, fragments, concrete abstract-type selections, conditional field presence,
variables/defaults, nullability and queries/mutations are supported. Subscription
functions require explicit enablement and a `SubscriptionTransport`; the bundled
`createGraphqlSseTransport` implements distinct-connection graphql-sse. Local
introspection JSON, full-file imports and direction-specific scalar callbacks are
supported. Experimental defer/stream uses a separate raw-only incremental output
and multipart deferSpec=20220824. WebSockets, multiplexing/reconnect, newer
ID-based incremental protocols and custom executable directives remain unsupported.
See the [advanced capability guide](graphql-capabilities.md). See the [support matrix](../../plugin-support-matrix.md) and
[remaining work](../inputs/native-pipelines.md#remaining-work).

Changes in this checkout are not part of the already published alpha.1 packages.

## Generated source layout

`graphql.ts` remains the public barrel. Individual model and operation files live in `graphql/models/` and `graphql/operations/`. `graphql-client.ts` constructs the client using bounded helper parts in `graphql/client/`; runtime code stays in `graphql-runtime.ts`. Export indexes are split into small parts. Model modules import only the input types they reference.

The layout targets source files below **128 KiB**, grouping declarations and export
parts at semantic boundaries. An indivisible model or operation that exceeds this
budget is retained and listed in `.poolster/source-layout-diagnostics.json`;
this is a size diagnostic, not a claim that every possible schema produces small files.
Regeneration removes obsolete unchanged owned files and preserves unrelated user files.
Source customizations referring to old monolithic paths must be retargeted.
