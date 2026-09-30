# Getting started with the Rust API

> Prefer the structured [Rust library documentation](library/README.md) for an
> onboarding path and [the embedded example](../examples/rust-embedded/README.md)
> for a runnable application. This page remains the detailed API reference.

For a command-line workflow without writing Rust configuration, use the
[CLI guide](cli.md). This guide covers native package composition.

Kaji has a bundled Go OpenAPI compiler and Rust SDK generators. Compile a local
Swagger 2.0 or OpenAPI 3.0/3.1 JSON or YAML document once, then generate as many packages as
needed from its artifacts.

## Add dependencies

Until the crates are published, use paths to a Kaji clone:

```toml
[dependencies]
anyhow = "1"
kaji = { path = "../kaji/crates/kaji" }
kaji-core = { path = "../kaji/crates/kaji-core" }
```

Use the Rust version declared by the workspace (currently Rust 1.85 or newer).
Go is required to build/run the source compiler, not to use a generated SDK.

## Compile the document

From the Kaji repository:

```sh
cd openapi
go run . --out ../.kaji/openapi ../openapi.yaml
```

The output includes normalized operations, schemas, and `security-schemes.json`.
Keep this artifact directory together; the Rust adapter requires the current
compiler format. It has no dependency on another repository or running service.

## Generate packages

In your Rust application, use an artifact path relative to its working directory:

```rust
use std::path::Path;
use anyhow::Result;
use kaji::{go, mock, prelude::*, python, rust, ts};

fn main() -> Result<()> {
    let release = ProfileSet::new("sdk")
        .package(ts::package("typescript/fetch")
            .name("@acme/email").with(ts::sdk().fetch().client_name("Email")))
        .package(ts::package("typescript/axios").with(ts::sdk().axios()))
        .package(rust::package("rust").with(rust::sdk()))
        .package(go::package("go").with(go::sdk()))
        .package(python::package("python").with(python::sdk()))
        .package(mock::package("mock-server").with(mock::server()));

    let tree = kaji::generate_openapi(
        Path::new("../kaji/.kaji/openapi"),
        "Email",
        "1.0.0",
        release,
    )?;
    tree.write_to("generated")?;
    Ok(())
}
```

This produces isolated packages below `generated/sdk`:

```text
sdk/
  typescript/
    fetch/
    axios/
  rust/
  go/
  python/
  mock-server/
```

Add `php::package(...).with(php::sdk())`, `java`, `csharp`, `elixir`, `ruby`, or `swift` in the
same way. Directory names and package names are separate choices.

Each SDK has its own README, manifest, and generated client. Install its
dependencies and build it using the target ecosystem's tooling. Generation
does not install those dependencies or publish anything.

## Generate from a Rust API model

If an integration already has a `kaji_core::Api`:

```rust
use anyhow::Result;
use kaji::{prelude::*, ts};
use kaji_core::{Api, SecuritySchemeCatalog};

fn generate_packages(api: &Api, catalog: &SecuritySchemeCatalog) -> Result<()> {
    let release = ProfileSet::new("sdk")
        .package(ts::package("typescript").with(ts::sdk()));
    let tree = kaji::generate_with_security_catalog(api, release, Some(catalog))?;
    tree.write_to("generated")?;
    Ok(())
}
```

For APIs without named security requirements, `kaji::generate(api, release)`
is sufficient. Do not infer credential behavior from a security scheme's name;
pass its catalog or load compiler artifacts.

The API model uses typed `request_body`, `responses`, and schemas. For example,
`OperationRequestBody::json(schema, true)` constructs a required JSON body;
`OperationResponse::json("200", schema)` constructs a JSON response.

## Regeneration

Generated files are replaced. Explicit custom starter files, including
TypeScript `custom/index.ts`, are created only if absent. Unrelated and obsolete
files are not deleted. Use a new output directory when removing/renaming
operations or changing output paths, then review the result before replacing a
published package. The writer checks relative paths and refuses symlink escapes;
a filesystem failure can still leave a partially written result.

Next: [all configuration options](configuration.md),
[raw versus full SDKs](generated-sdks.md), or
[Zod and frontend artifacts](auxiliary-generators.md).
