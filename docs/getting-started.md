# Getting started with the Rust API

> Prefer the structured [Rust library documentation](library/README.md) for an
> onboarding path and [the embedded example](../examples/rust-embedded/README.md)
> for a runnable application. This page remains the detailed API reference.

For a command-line workflow without writing Rust configuration, use the
[CLI guide](cli.md). This guide covers native package composition.

## Choose the input path

| Source | Entry point | Current result |
| --- | --- | --- |
| OpenAPI / Swagger | Compile artifacts, then `generate_openapi` | Existing HTTP SDK packages. |
| An in-memory `Api` | `generate` or `generate_with_security_catalog` | Existing HTTP SDK packages. |
| Registered HTTP input | `generate_with_input` with an `AdaptedApi` capability | Existing HTTP SDK packages. |
| GraphQL, events, workflows or RPC | `InputRegistry` and `InputProvider<C>` | Native contracts for inspection and custom output consumers. |

Follow [input plugins](input-plugins.md) for native format support and the
input-to-output hook. The steps below use the bundled Go OpenAPI compiler.

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

Call `tree.check(directory)` to review drift before writing. `write_to` updates
unchanged owned files, removes unchanged obsolete owned files and rejects local
edits to owned output. Unrelated files and create-once starters such as
TypeScript `custom/index.ts` are preserved.

The writer rejects unsafe paths and symlink escapes. Filesystem failures can
still interrupt materialization. See [safe regeneration](safe-regeneration.md)
for ownership, legacy adoption and conflict resolution.

Next: [all configuration options](configuration.md),
[raw versus full SDKs](generated-sdks.md), or
[Zod and frontend artifacts](auxiliary-generators.md).
