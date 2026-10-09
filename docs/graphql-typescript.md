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
      "scalars": { "DateTime": { "input": "string", "output": "string" } }
    }]
  }]
}
```

Paths resolve relative to the recipe. Run `poolster generate --config poolster.json`.
Build the emitted package using its pinned TypeScript dependency, then import its
compiled package exports. The [runnable example](../examples/graphql-native/README.md)
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
    scalars: { DateTime: { input: 'string', output: 'string' } },
  },
  output: './generated',
  plugins: [pluginTypeScript({ path: 'client', name: '@example/graphql-client' })],
});
```

Config-file loading resolves operation paths alongside the config file. This
For mixed TypeScript/Rust outputs, provide Rust mappings separately as
`input.rustScalars`; `input.scalars` remains the TypeScript mapping surface. This
entry point invokes the native GraphQL generator; it does not establish general
Node dispatch for Rust contract/block hooks.

## Scalar mappings

Each custom scalar mapping has separate `input` and `output` TypeScript types.
Input mappings apply to variables and input-object fields; output mappings apply
to selected results. GraphQL nullability, list element nullability and optional
presence remain separate from the scalar mapping. Unconfigured scalars stay
`unknown`. Mapping names must match custom scalars reachable from the generated
contract; unknown/unused names and overrides of built-in scalars are rejected.

Mappings describe JSON wire values; they do not add runtime codecs. An output
mapping of `Date` does not convert a JSON string into a JavaScript date. Use a
wire-compatible type such as `string`, and convert it in application code.

Through the Rust API, configure the same generator:

```rust
let output = ts::graphql(Some(input.handle()))
    .scalar("DateTime", ts::GraphqlScalarMapping::new("string", "string"));
```

## Results and transport failures

```ts
import { createGraphqlHttpTransport, Viewer } from '@example/graphql-client';
const transport = createGraphqlHttpTransport('http://localhost:4000/graphql');
const result = await Viewer(transport, {});
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

## Supported boundary

Aliases, fragments, concrete abstract-type selections, conditional field presence,
variables/defaults, nullability and queries/mutations are supported. Subscription
functions require explicit enablement and an injected `SubscriptionTransport`;
a bundled WebSocket/SSE transport is not included. Introspection/schema imports,
custom executable directives, defer/stream and runtime scalar codecs remain
outside this increment. See the [support matrix](plugin-support-matrix.md) and
[remaining work](native-pipelines.md#remaining-work).

Changes in this checkout are not part of the already published alpha.1 packages.
