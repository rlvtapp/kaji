# Getting started

Kaji is a Rust library today—not a CLI and not a Node package. You give it a
normalized API model (normally the output of Relevate Docs' existing OpenAPI
sidecar), select the packages you want, then materialize the returned files.

For a friend trying Kaji from this repository, add the two workspace crates to
their Rust project with paths that point at their clone:

```toml
[dependencies]
anyhow = "1"
kaji = { path = "../Kaji/crates/kaji" }
kaji-core = { path = "../Kaji/crates/kaji-core" }
```

Kaji requires Rust 1.85 or newer. The generated SDKs have their own native
toolchain requirements; see [generated SDKs](generated-sdks.md).

## Generate from the Docs OpenAPI sidecar

This is the usual Relevate integration. Run the Docs compiler's existing Go
OpenAPI sidecar first, then hand its completed output directory to Kaji.

```rust
use std::path::Path;

use anyhow::Result;
use kaji::{ProfileSet, generate_openapi};

fn main() -> Result<()> {
    let artifacts = generate_openapi(
        Path::new(".cache/openapi-sidecar"),
        "Relevate Email",
        "2026.9.25",
        ProfileSet::new("sdk")
            .rust()
            .typescript_fetch()
            .typescript_axios()
            .go()
            .python()
            .php()
            .java()
            .dotnet()
            .elixir()
            .mock_server(),
    )?;

    artifacts.write_to("generated")?;
    Ok(())
}
```

That produces one independently usable package per selected target:

```text
generated/
  sdk/
    rust/
    typescript-fetch/
    typescript-axios/
    go/
    python/
    php/
    java/
    dotnet/
    elixir/
    mock-server/
```

`generate_openapi` reads `operations.json`, `operations-order.json`, the
operation files below `operations/`, optional `schemas.json`, and optional
`security-schemes.json` from that sidecar directory. It keeps declared
security, request/response schemas, examples, and Kaji extensions intact.

## Generate from your own Rust adapter

If your project already has a `kaji_core::Api`, skip the sidecar:

```rust
use anyhow::Result;
use kaji::{ProfileSet, generate};
use kaji_core::Api;

fn generate_packages(api: &Api) -> Result<()> {
    let artifacts = generate(
        api,
        ProfileSet::new("sdk")
            .typescript_fetch()
            .python()
            .mock_server(),
    )?;
    artifacts.write_to("generated")?;
    Ok(())
}
```

`GeneratedTree::write_to` only accepts safe relative generated paths and
refuses symlink escapes. It overwrites generated files on a later run. The one
exception is TypeScript's `custom/index.ts`: Kaji creates it once and leaves
subsequent user edits untouched.

## The three things to decide

1. Choose target packages with `ProfileSet`.
2. Choose the public SDK shape: namespaced by default, flat where preferred.
3. Optionally generate the language-neutral mock package alongside the SDKs.

The [configuration reference](configuration.md) lists every available option.
The [generated SDK guide](generated-sdks.md) explains what consumers receive.
