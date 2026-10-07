# Embed generation and develop plugins in Rust

Use the Rust library when generation belongs inside your own tool, or when you need a plugin that the CLI's bundled registry does not expose. It gives you typed packages, provider/consumer contracts, and a virtual file tree. Your application decides when to inspect, check, or write that tree.

For SDK authors who only need a recipe, bundled middleware, and releases, start with the [CLI workflow](../cli/README.md). The library uses the same renderers and output safety rules; embedding is an integration choice rather than a different SDK format.

## Prerequisites and first result

You need a Rust toolchain compatible with the workspace and access to a Kaji checkout. The [quickstart](quickstart.md) uses local Cargo path dependencies; do not assume every workspace crate is published to crates.io. For its source compiler step, you also need Go. [OpenAPI compiler](../openapi-compiler.md) explains the artifacts consumed by `generate_openapi`.

Start with [the library quickstart](quickstart.md), or copy [the embedded Rust example](../../examples/rust-embedded/README.md). The result is a program that reads compiler artifacts, composes packages, and writes an SDK tree. Generation does not install output dependencies or publish anything; run the generated package's native build and tests afterwards.

The public Rust API is pre-1.0. Pin compatible revisions/versions and review [release notes](../releases/0.4.0.md) when updating.

## SDK authors: compose, customize, and verify

Packages own language settings, identity, and directories. Plugins own rendering choices. A simple composition looks like this:

```rust
use kaji::{prelude::*, ts};

let release = ProfileSet::new("sdk")
    .package(ts::package("typescript")
        .name("@acme/api")
        .with(ts::sdk().fetch())
        .with(ts::zod()));

let tree = kaji::generate(&api, release)?;
let drift = tree.check("generated")?;
if !drift.is_empty() {
    // Inspect the proposed changes before materializing them.
    tree.write_to("generated")?;
}
```

This fragment assumes an already-normalized `api` and an enclosing function returning a compatible `Result`. It checks/writes below `generated/sdk`; it does not run a build. Follow the [quickstart](quickstart.md) for a complete executable program and [configuration reference](../configuration.md) for package/plugin settings.

[SDK customization](../sdk-customization.md) shows the library equivalents of author source overlays and bundled middleware. [Safe regeneration](../safe-regeneration.md) explains how owner identities and hashes protect materialized files. Use native build/behavioral checks for your API and customization; [shared fixtures](../shared-sdk-fixtures.md) can supply bounded schema samples, while [verification](../verification.md) explains their practical limits.

When you are ready to deliver packages, [SDK automation](../sdk-automation.md) explains optional release metadata and [publishing](../sdk-publishing.md) explains tagged registry artifacts. A native/community plugin can supply that metadata without adding a registry implementation to core.

## Plugin developers: learn the contracts before the renderer

First follow [plugin composition](plugins.md) to combine maintained providers and consumers. Then read [typed plugins](../typed-plugins.md) for the `Language`/`Plugin` APIs, contract handles, dependencies, execution phases, and finalization. Missing inputs, ambiguous providers, cycles, and conflicting paths are generation errors; ordering alone is not a substitute for declaring a dependency.

Use [architecture](../architecture.md) and the [compiler guide](../openapi-compiler.md) to understand the source-model boundary. Keep target-specific semantics in the language/plugin. Publish typed contracts for reusable results instead of requiring another plugin to scrape generated source. [Native SDK providers](../native-sdk-providers.md) shows existing Rust/Go composition and transport ABI limits; [auxiliary generators](../auxiliary-generators.md) shows consumers that add artifacts around an SDK.

A native plugin is registered by composing it in your Rust application. It is not automatically available as a `kaji.json` plugin name. To extend that registry or change delivery action sources, follow [source customization](../source-customization.md).

## Choose checks that prove the intended behavior

Compile a representative generated package, execute its custom transport/policy, and test request/response/error behavior. Snapshot checks prove output stability; schema roundtrips prove selected codec cases. Neither alone proves authentication, pagination, cancellation, or a live registry upload. The [verification overview](../verification.md), [native provider boundaries](../native-sdk-providers.md), and [testing guide](../guides/testing.md) help choose coverage.

The facade also exposes `postman` and `terraform` plugins. [Postman collections](../postman.md) use typed example/document/environment contracts; [Terraform](../terraform-provider.md) separates entity analysis from typed provider rendering. Read each guide for the initial supported subset and plugin composition examples.
