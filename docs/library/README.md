# Generate from Rust

Use the Rust library when generation belongs inside another tool, service, or
build process. It offers typed package composition and native Rust plugins while
using the same compiler and output writer as the CLI.

1. [Library quickstart](quickstart.md) — compile a contract and emit packages.
2. [Plugin composition](plugins.md) — compose maintained plugins or write one.
3. [Configuration reference](../configuration.md) — public settings.
4. [Generated SDK behavior](../generated-sdks.md) — understand the output.

Copy [examples/rust-embedded](../../examples/rust-embedded/README.md) for a
small standalone application. The public Rust API is pre-1.0, so pin compatible
versions and follow release notes for changes.
