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

## Generated source layout

Generated sources separate `GraphqlRuntime.java`, `Client.java`, `models/`, `operations/` and `groups/`. Operation/raw/grouped call paths are unchanged. **Model imports change:** use `<package>.models.ReadUserVariables` instead of `Client.ReadUserVariables`. The generated README describes this migration. Public class filenames follow Java rules; unsupported long/colliding names reject before writing.

The layout targets source files below **128 KiB**, grouping declarations and export
parts at semantic boundaries. An indivisible model or operation that exceeds this
budget is retained and listed in `.poolster/source-layout-diagnostics.json`;
this is a size diagnostic, not a claim that every possible schema produces small files.
Regeneration removes obsolete unchanged owned files and preserves unrelated user files.
Source customizations referring to old monolithic paths must be retargeted.
