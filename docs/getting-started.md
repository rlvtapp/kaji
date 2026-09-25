# Getting started

Kaji has two local pieces: the embedded Go OpenAPI compiler in `openapi/`, and
the Rust SDK generator crates. Compile an OpenAPI 3.0/3.1 JSON or YAML document
into Kaji artifacts, select packages, then materialize the generated files.

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

## Compile an OpenAPI document

The checked-in Go module is Kaji's OpenAPI compiler. Run it from the repository
root; it accepts JSON or YAML and writes a deterministic artifact directory.

```sh
cd openapi
go run . --out ../.kaji/openapi ../openapi.yaml
```

The output contains normalized operation documents, component schemas, and
security scheme metadata. It is an internal Kaji boundary, not a dependency on
Relevate Docs or another repository.

## Generate SDK packages

```rust
use std::path::Path;

use anyhow::Result;
use kaji::{ProfileSet, generate_openapi};

fn main() -> Result<()> {
    let artifacts = generate_openapi(
        Path::new(".kaji/openapi"),
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

`generate_openapi` reads the local compiler artifacts. It preserves component
schemas, request/response media, security schemes, examples, extensions, and
operation metadata in Kaji's Rust AST without requiring a Go service at runtime.

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
