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

Custom scalars initially use `serde_json::Value`; TypeScript scalar expressions
cannot be reused as Rust types. Runtime scalar codecs and subscription transports
are outside this initial Rust increment.

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
  },
  output: './generated',
  plugins: [pluginRust({ path: 'client', name: 'example-graphql-client' })],
});
```

HTTP and GraphQL generators require separate Rust packages. Rust custom scalar
mappings are not yet supported; generate a separate TypeScript configuration if
it supplies TypeScript scalar mappings.

The two explicit native tests pass: deterministic regeneration, and `.crate`
pack/unpack plus clean consumer compilation/execution against a local GraphQL
server. They cover defaults, omitted/null/value inputs, conditional selected
fields, required nullable results, partial/errors and transport failures. Negative
consumer checks reject a wrongly typed variable and an unselected result field.
See [verification](verification.md#graphql-client-completion-checks).

Abstract selections without `__typename` use strict structural variants; ambiguous
identical-key variants with different types require selecting `__typename`.
