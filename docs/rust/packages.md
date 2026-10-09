# Compose packages

← [Rust SDK](README.md)

Each package has a language, plugins and its own contract graph.

```rust
let profiles = ProfileSet::new("sdk")
    .package(rust::package("rust-client").with(rust_output).with(rust_input))
    .package(ts::package("web-client").with(ts_output).with(ts_input));
```

Imports and input setup follow the [quickstart](quickstart.md). Install only the
language crates you use; `ts` above is `poolster_plugin_typescript`.

## Select a provider

```rust
let client = ts::graphql(Some(input.handle())).flat();
let selected = client.handle();
let package = ts::package("web")
    .with(ts::zod().using_graphql(Some(selected)))
    .with(client)
    .with(input);
```

A handle selects that exact provider. Dependencies determine execution order,
so registering the consumer first is valid. Multiple providers of the same
contract require explicit selection. Handles cannot cross package boundaries.

Outputs must support their input contract. OpenAPI uses HTTP contracts; GraphQL,
RPC, events and workflows retain their own semantics.

**Next:** [Check and write](files.md) · [Provider composition reference](../internals/typed-plugins.md)
