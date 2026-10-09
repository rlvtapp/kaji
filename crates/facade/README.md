# Poolster Rust API

Compose language-scoped SDK packages from one API contract.

```rust
use poolster::{mock, prelude::*};
use poolster_plugin_go::PackageExt as _;
use poolster_plugin_go as go;
use poolster_plugin_rust::PackageExt as _;
use poolster_plugin_rust as rust;
use poolster_plugin_typescript::PackageExt as _;
use poolster_plugin_typescript as ts;

let release = ProfileSet::new("sdk")
    .common(Common::default().package_version("1.0.0"))
    .package(ts::package("typescript")
        .name("@acme/sdk")
        .with(ts::sdk().axios().client_name("Acme")))
    .package(rust::package("rust").with(rust::sdk()))
    .package(go::package("go").with(go::sdk().flat().jobs(4)))
    .package(mock::package("mock-server").with(mock::server().port(5000)));

let tree = poolster::generate(&api, release)?;
tree.write_to("generated")?;
```

Use `generate_openapi(path, name, version, release)` for the artifacts emitted
by the bundled Go compiler. For an in-memory API with named security requirements,
pass its definitions to `generate_with_security_catalog`.

## Custom input adapters

`poolster_core::Adapter` is the input extension point. It returns an `AdaptedApi`
containing Poolster's neutral `Api` plus its named security catalog, so language
plugins remain independent of the source format:

```rust
use anyhow::Result;
use poolster::{ProfileSet, generate_with_adapter};
use poolster_plugin_typescript as ts;
use poolster_core::{AdaptedApi, Adapter, Api, SecuritySchemeCatalog};

struct CompanyContract;

impl Adapter for CompanyContract {
    fn adapt(&self) -> Result<AdaptedApi> {
        Ok(AdaptedApi::new(Api::default(), SecuritySchemeCatalog::default()))
    }
}

let tree = generate_with_adapter(
    &CompanyContract,
    ProfileSet::new("sdk").package(ts::package("typescript").with(ts::sdk())),
)?;
```

The bundled OpenAPI path is an `OpenApiSidecar` adapter and remains available
through `generate_openapi`. Output extension remains language plugins; there is
no separate output-parser interface to implement.

Add each language plugin as a dependency and import its crate explicitly. Set package identity with
`.name(...)`, shared defaults with `.common(...)`, and individual generator
options on `.with(language::sdk()...)`.

SDKs are namespaced by default; `.flat()` selects direct clients. TypeScript
also supports `.raw()`, which emits operation functions without the class.
Select Fetch and Axios in different packages when you need both.

- [Getting started](../../docs/rust/quickstart.md)
- [All configuration options](../../docs/reference/configuration/configuration.md)
- [Generated SDK surfaces](../../docs/reference/outputs/generated-sdks.md)
- [Zod, TanStack, and other artifacts](../../docs/reference/outputs/auxiliary-generators.md)
- [Plugin authoring](../../docs/internals/typed-plugins.md)
- [Contract mocking](../../docs/reference/outputs/mocking.md)
