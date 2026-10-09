# GraphQL → Python clients

Unreleased. Generate selection-specific query/mutation clients from a schema and
operation documents. Models are `TypedDict` definitions, not schema-wide classes.
Python 3.9+ and the standard library are sufficient for the runtime.

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language python --output generated
```

The existing npm `pluginPython` also supports GraphQL inputs:

```js
plugins: [pluginPython({
  path: 'python', name: 'example-graphql-client',
  contracts: { graphql: { style: 'flat' } },
})]
```

## Use the generated package

For an operation named `ReadUser`, install the generated package, then:

```python
from example_graphql_client import Client

client = Client('http://localhost:4000/graphql', timeout=10)
response = client.read_user({'id': '42'})
if response.errors:
    # Data can still contain usable partial results.
    print(response.errors)
if response.data is not None:
    print(response.data)
```

Replace the module name and variables with your generated package and operation.
Typed dictionaries retain GraphQL field names on the wire. Omit optional keys to
send absence; pass `None` for explicit null. Optional-only variables may be
omitted from bound calls.

`raw` provides functions in `operations`; `flat` adds direct snake_case client
methods. `idiomatic` adds `client.query.read_user(...)` and mutation groups while
retaining direct methods. Explicit group mappings can expose
`client.users.read(...)`.

## Configure groups from Rust

```rust
let output = python::graphql(Some(input.handle()))
    .idiomatic()
    .group("users", "read", "ReadUser");
```

The CLI JSON recipe accepts `groups` and `style` on its `graphql` or `sdk` plugin,
including through `contracts.graphql`. The operation string is the document's
operation name, not a schema field guessed by Poolster.

## Errors and limits

The response envelope distinguishes errors, partial data and whether the `data`
key was present. Network failures, invalid envelopes and non-GraphQL HTTP errors
raise exceptions; valid GraphQL errors remain in `response.errors`. Selected model
annotations are static typing information, not runtime schema validation.

Synchronous HTTP only. Subscriptions, incremental delivery, dynamic caller-selected
fields and custom scalar mappings are unsupported. Unmapped scalars use `Any`.
The default transport uses urllib; it does not retry mutations automatically.

Tests include generated Python compilation, presence/null cases, regeneration,
recursive inputs, grouped methods, and pinned GraphQL.js 16.14.2 local-server calls.
See the [support matrix](../../plugin-support-matrix.md).
