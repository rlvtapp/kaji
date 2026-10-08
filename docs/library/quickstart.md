# Rust library quickstart

The library consumes normalized compiler artifacts and returns a virtual file
tree. Your application chooses when and where to write it.

## Add Poolster

Until registry publishing is available, use paths from a Poolster clone:

```toml
[dependencies]
anyhow = "1"
poolster = { path = "../kaji/crates/kaji" }
poolster-plugin-typescript = { path = "../kaji/crates/plugins/typescript" }
poolster-plugin-go = { path = "../kaji/crates/plugins/go" }
```

## Compile and generate

This example generates HTTP SDKs from OpenAPI compiler artifacts. For GraphQL,
event, workflow or RPC sources, use the [input provider path](../input-plugins.md).

```sh
cd openapi
go run . --out ../.poolster/openapi ../openapi.yaml
```

```rust
use std::path::Path;
use anyhow::Result;
use poolster::prelude::*;
use poolster_plugin_go as go;
use poolster_plugin_typescript as ts;

fn main() -> Result<()> {
    let packages = ProfileSet::new("sdk")
        .package(ts::package("typescript")
            .name("@acme/pet-store")
            .with(ts::sdk().fetch().client_name("PetStore")))
        .package(go::package("go").with(go::sdk()));
    let files = poolster::generate_openapi(
        Path::new(".poolster/openapi"), "Pet Store", "1.0.0", packages,
    )?;
    files.write_to("generated")?;
    Ok(())
}
```

## Verify the output

Use `files.check("generated")?` to inspect drift before writing.
Generation does not install dependencies or publish packages. Build each output
with its target-native tooling. Continue with [plugin composition](plugins.md)
or the full [embedded-generation reference](../getting-started.md).
