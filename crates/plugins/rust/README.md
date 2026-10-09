# Poolster Rust plugin

Generate a Cargo SDK with Serde models, a Reqwest client, typed operation
requests, and resource accessors.

```rust
use poolster::prelude::*;
use poolster_plugin_rust::PackageExt as _;
use poolster_plugin_rust as rust;

let release = ProfileSet::new("sdk")
    .package(rust::package("rust")
        .name("email-sdk")
        .with(rust::sdk().namespaced()));
let tree = poolster::generate(&api, release)?;
tree.write_to("generated")?;
```

Namespaced is the default; `.flat()` keeps direct operations only.
`.operation_prefix("api")` prefixes direct method names and the resource
delegates that call them. Package `.name(...)` controls the Cargo crate name.

Operations with parameters accept typed request structs. Pass values, not a
completed URL: the client escapes path parameters and serializes query values.
Request bodies are separate arguments. Exact method names and signatures are
documented in each generated package.

For an in-memory API with named security schemes, supply its security catalog;
`poolster::generate_openapi` loads the bundled compiler's catalog automatically.

[Configuration reference](../../../docs/reference/configuration/configuration.md) ·
[Generated SDKs](../../../docs/reference/outputs/generated-sdks.md)

Operations and resource facades group complete request/error/method declarations
under a 128 KiB byte budget, in addition to the existing declaration-count limit.
Model structs retain their native public fields in individual files. An atomic
struct, enum or operation larger than the budget is retained intact and reported
in `.poolster/source-layout-model-diagnostics.json` or
`.poolster/source-layout-operation-diagnostics.json`. This avoids changing field access
or replacing typed models with generic JSON to meet a physical file limit.
Generated-file ownership protects local edits during regeneration.
