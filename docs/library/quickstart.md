# Rust library quickstart

The library consumes normalized compiler artifacts and returns a virtual file
tree. Your application chooses when and where to write it.

## Add Kaji

Until registry publishing is available, use paths from a Kaji clone:

```toml
[dependencies]
anyhow = "1"
kaji = { path = "../kaji/crates/kaji" }
```

## Compile and generate

```sh
cd openapi
go run . --out ../.kaji/openapi ../openapi.yaml
```

```rust
use std::path::Path;
use anyhow::Result;
use kaji::{go, prelude::*, ts};

fn main() -> Result<()> {
    let packages = ProfileSet::new("sdk")
        .package(ts::package("typescript")
            .name("@acme/pet-store")
            .with(ts::sdk().fetch().client_name("PetStore")))
        .package(go::package("go").with(go::sdk()));
    let files = kaji::generate_openapi(
        Path::new(".kaji/openapi"), "Pet Store", "1.0.0", packages,
    )?;
    files.write_to("generated")?;
    Ok(())
}
```

Generation does not install dependencies or publish packages. Build each output
with its target-native tooling. Continue with [plugin composition](plugins.md)
or the full [embedded-generation reference](../getting-started.md).
