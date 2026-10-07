# Native plugin composition

Kaji plugins are typed Rust implementations. They render SDKs, consume declared
outputs from other plugins, or run after generation to add derived files. Missing
inputs, duplicate providers, cycles, and conflicting paths are errors.

```rust
use kaji::{prelude::*, ts};

let package = ts::package("typescript")
    .name("@acme/pet-store")
    .with(ts::sdk().fetch())
    .with(ts::composition::zod().output("validation"));
```

Use one package per transport and layout. Package configuration owns names and
directories; plugin configuration owns rendering behavior. The full
[plugin-authoring reference](../typed-plugins.md) explains contracts, handles,
post phases, dependencies, and current composition boundaries.
