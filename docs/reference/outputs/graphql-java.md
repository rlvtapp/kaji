# GraphQL → Java clients

Unreleased. Generated Java 17 clients use records, JDK HTTP and Jackson 2.18.3.
Maven and Gradle manifests are emitted with the schema and operation documents.

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language java --output generated
```

Select the same pipeline with npm `pluginJava({ contracts: { graphql: { style: 'flat' } } })`
or Rust `java::graphql(Some(input.handle())).flat()` inside a Java package.

## Styles and results

Raw exposes static operation methods; flat exposes methods directly on the client.
Grouped exposes query/mutation accessors. Explicit groups map a method to an operation:
`groups: { users: { read: 'ReadUser' } }`. Generated package names follow package configuration.
Use the emitted operation-specific variable records and inspect the returned response's
data and errors together: a resolver error can coexist with usable partial data.
The transport is injectable; HTTP and JSON failures are separate from GraphQL errors.

Records model only selected fields. Optional presence distinguishes omitted values
from explicit null, including nested inputs. Abstract selections use concrete variants
and require selected `__typename`; custom scalars use `JsonNode`. Enums retain wire values.
Subscriptions and incremental delivery are unsupported. Invalid or colliding Java
identifiers and records exceeding 250 fields reject before generation.

All four styles compile and execute against pinned GraphQL.js 16.14.2, including
mutations, nested nullable lists, input presence and abstract variants.
See the [support matrix](../../plugin-support-matrix.md).
