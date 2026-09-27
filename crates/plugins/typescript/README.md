# Kaji TypeScript plugin

Generate a Fetch or Axios SDK, a standalone types package, or auxiliary
validation/frontend artifacts. Generation runs entirely in Rust.

```rust
use kaji::{prelude::*, ts};

let release = ProfileSet::new("sdk")
    .package(ts::package("typescript")
        .name("@acme/email")
        .with(ts::sdk()
            .axios()
            .client_name("Email")
            .model_options(ts::ModelOptions {
                enum_type: ts::EnumType::AsConst,
                ..Default::default()
            })));
let tree = kaji::generate(&api, release)?;
tree.write_to("generated")?;
```

Fetch is the default transport. `.raw()` removes the instantiated class but
keeps direct operations and models. Full clients default to namespaced;
`.flat()` selects direct class methods. Transport choices belong to the SDK
plugin; use separate packages for Fetch and Axios.

`ts::types()` emits models and publishes the `TsTypes` symbol contract for
community consumers. Do not combine it with `ts::sdk()` in one package.

`ts::artifacts` exposes Zod, TanStack React/Vue Query, SWR, Faker, MSW, Cypress,
ReDoc, and MCP manifest renderers. These return files directly, not ready-made
typed plugins. They are not CLI targets. Review their dependencies, import
configuration, and limitations before using generated output.

- [Every SDK/model option](../../../docs/configuration.md#typescript-sdk)
- [Raw and full client usage](../../../docs/generated-sdks.md)
- [Auxiliary artifact API and limitations](../../../docs/auxiliary-generators.md)
- [Contracts and plugin composition](../../../docs/typed-plugins.md)
