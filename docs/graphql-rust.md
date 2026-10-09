# GraphQL → Rust client generation

Implemented and verified in this checkout; not yet published. The
[support matrix](plugin-support-matrix.md) records the supported boundary.

The Rust generator consumes the same `GraphqlOperations` contract as TypeScript,
without converting operations into HTTP `Api` endpoints. Each query/mutation has
its own selected result and variables types. Generated types use Serde, and HTTP
execution uses a caller-configured Reqwest client.

## Generate with the CLI

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language rust --output generated
```

For package naming/versioning, use the same input recipe as TypeScript with
`language: "rust"` and a `graphql` output plugin. A recipe can include separate
TypeScript and Rust packages; each consumes the same validated operations.
Rust rejects TypeScript scalar expressions and subscription options explicitly.

## Rust plugin use

```rust
let generator = rust::graphql(Some(input.handle()));
let package = rust::package("client").with(input).with(generator);
```

A named operation `Read` exposes `ReadVariables`, `ReadResult` and an async `read`
function. The HTTP transport accepts an endpoint and a Reqwest client, allowing
application headers/timeouts to remain caller-controlled. Dropping the future
cancels the caller's request rather than automatically retrying mutations.

GraphQL responses distinguish success, usable partial data with errors, and
errors without data. Network/HTTP/protocol/decoding failures are separate from
GraphQL application errors.

## Presence and wire semantics

Optional nullable fields use three states: absent, explicit null and a value.
Optional non-null fields distinguish absent and a value. Required nullable result
fields must be present, even when null. Enum values retain their GraphQL wire
strings, and selected abstract variants retain their selected fields.

Unmapped custom scalars use `serde_json::Value`. Configure independent Rust input
and output wire types when the server's representations are known:

```rust
let generator = rust::graphql(Some(input.handle()))
    .scalar("Timestamp", rust::GraphqlScalarMapping::new("String", "i64"));
```

The same recipe plugin accepts language-specific mappings:

```json
{
  "language": "rust",
  "path": "client",
  "plugins": [{
    "name": "graphql",
    "scalars": { "Timestamp": { "input": "String", "output": "i64" } }
  }]
}
```

Mappings apply recursively to variables/input objects and selected results,
including lists. GraphQL presence and nullability wrappers remain independent.
Supported self-contained types are `String`, primitive numbers/bool,
`serde_json::Value`, and nested `Vec<T>`, `Option<T>` or `BTreeMap<String, T>`.
The supported containers and `String` also accept their `std`-qualified paths;
emitted types use canonical paths to avoid generated-name collisions.

Unknown/unused scalar names, builtin overrides, malformed expressions and
unsupported imported types fail before output is written. Borrowed types, custom
structs and dependencies such as Chrono are not supplied by this mapping API.
Mappings describe JSON wire values and install no codecs: `String` input and
`i64` output work only if the server accepts strings and returns integers.
An incompatible response produces a `GraphqlTransportError::Decode`.
Subscriptions remain outside this increment.

## Node entry point

The existing Rust plugin package can select this generator from native GraphQL
input without an HTTP conversion:

```js
import { generate } from '@relevate/poolster';
import { inputGraphql } from '@relevate/poolster-input-graphql';
import { pluginRust } from '@relevate/poolster-plugin-rust';

await generate({
  input: {
    path: './schema.graphql',
    plugin: inputGraphql(),
    operations: ['./operations.graphql'],
    rustScalars: { Timestamp: { input: 'String', output: 'i64' } },
  },
  output: './generated',
  plugins: [pluginRust({ path: 'client', name: 'example-graphql-client' })],
});
```

HTTP and GraphQL generators require separate Rust packages. Node configuration
uses `input.rustScalars` for Rust and `input.scalars` for TypeScript, allowing mixed
language packages with independent mappings. TypeScript expressions are not
interpreted as Rust types.

The two explicit native tests pass: deterministic regeneration, and `.crate`
pack/unpack plus clean consumer compilation/execution against a local GraphQL
server. They cover defaults, omitted/null/value inputs, conditional selected
fields, required nullable results, partial/errors and transport failures. Negative
consumer checks reject a wrongly typed variable and an unselected result field.
See [verification](verification.md#graphql-client-completion-checks).

Abstract selections without `__typename` use strict structural variants; ambiguous
identical-key variants with different types require selecting `__typename`.

The packaged runtime test also verifies a real custom GraphQL scalar with
`String` input and `i64` output, nested/list/presence/nullability behavior, unmapped
JSON values and incompatible server data.
