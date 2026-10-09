# Generate from a Rust application

← [Rust SDK](README.md)

Add the crates you use. For the unreleased checkout, use local paths:

```toml
[dependencies]
anyhow = "1"
poolster = { path = "../poolster/crates/facade" }
poolster-core = { path = "../poolster/crates/core" }
poolster-input-openapi = { path = "../poolster/crates/inputs/openapi" }
poolster-plugin-rust = { path = "../poolster/crates/plugins/rust" }
```

## Generate a package

```rust
use anyhow::Result;
use poolster::prelude::*;
use poolster_core::AdaptedApi;
use poolster_input_openapi::OpenApiCompilerInput;
use poolster_plugin_rust as rust;
use std::sync::Arc;

fn main() -> Result<()> {
    let mut registry = InputRegistry::new();
    registry.register(OpenApiCompilerInput {
        executable: std::env::var_os("POOLSTER_OPENAPI_BIN")
            .unwrap_or_else(|| "poolster-openapi".into()).into(),
        name: "Example API".into(),
        version: "1.0.0".into(),
    })?;
    let input = InputProvider::<AdaptedApi>::new(
        Arc::new(registry), "openapi", "./openapi.yaml",
    ).using("openapi.compiler");
    let output = rust::sdk().input(input.handle());
    let profiles = ProfileSet::new("sdk")
        .package(rust::package("client").with(output).with(input));

    poolster::generate_native(profiles)?.write_to("generated")?;
    Ok(())
}
```

The OpenAPI provider uses Poolster's compiler executable. Supply its path through
`POOLSTER_OPENAPI_BIN`; the Rust library does not download a platform binary.
The generated client appears under `generated/sdk/client`.

The facade has no language plugins enabled by default. Depending directly on
`poolster-plugin-rust` keeps the selected generator explicit.

**Next:** [Compose packages](packages.md) · [Other input providers](../reference/inputs/input-plugins.md)
