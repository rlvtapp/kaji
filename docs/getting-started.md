# Getting started

Kaji is a Rust library today—not a CLI and not a Node package. It parses an
OpenAPI 3.0/3.1 JSON or YAML document natively, selects the requested packages,
and returns the generated files for materialization.

Until the crates are published, use path dependencies that point at a Kaji
clone:

```toml
[dependencies]
anyhow = "1"
kaji = { path = "../Kaji/crates/kaji" }
kaji-core = { path = "../Kaji/crates/kaji-core" }
```

Kaji requires Rust 1.85 or newer. The generated SDKs have their own native
toolchain requirements; see [generated SDKs](generated-sdks.md).

## Generate from an OpenAPI document

`generate_openapi_file` is the normal entry point for a checked-in
`openapi.yaml` or `openapi.json`. Kaji reads the document directly.

```rust
use std::path::Path;

use anyhow::Result;
use kaji::{ProfileSet, generate_openapi_file};

fn main() -> Result<()> {
    let artifacts = generate_openapi_file(
        Path::new("openapi.yaml"),
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

`generate_openapi_file` reads JSON or YAML, validates that the document is
OpenAPI 3.0 or 3.1, and preserves component schemas, request/response media,
security schemes, OpenAPI extensions, and operation metadata in Kaji's native
Rust AST. `generate_openapi` accepts the document bytes directly when the spec
comes from memory, an HTTP response, or another storage system.

## Generate from an existing Rust adapter

If an integration already has a `kaji_core::Api`, pass it directly:

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
