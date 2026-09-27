# Kaji Rust plugin

Generate a Cargo SDK with Serde models, a Reqwest client, typed operation
requests, and resource accessors.

```rust
use kaji::{prelude::*, rust};

let release = ProfileSet::new("sdk")
    .package(rust::package("rust")
        .name("email-sdk")
        .with(rust::sdk().namespaced()));
let tree = kaji::generate(&api, release)?;
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
`kaji::generate_openapi` loads the bundled compiler's catalog automatically.

[Configuration reference](../../../docs/configuration.md) ·
[Generated SDKs](../../../docs/generated-sdks.md)
