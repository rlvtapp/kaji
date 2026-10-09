# Customize and rebuild the generator

Eject the installed CLI's source workspace to customize renderers or register
Rust plugins. Rebuild that workspace to change future generated SDKs.
Customers use the generated package; they do not need the generator workspace.

```sh
poolster eject --language ruby --out ./my-poolster
cd my-poolster
```

**Use a new destination.** Eject writes source only; it does not overwrite files,
generate an SDK, download dependencies, publish or install GitHub automation.

| File | Purpose |
| --- | --- |
| `EJECTED.md` | Target-specific editing entrypoints |
| `EJECTED-SOURCES.json` | Original file SHA-256 hashes and generator version |

**On this page:** [Bundle](#what-gets-ejected) · [Build](#build-and-generate) · [Edit](#change-generated-behavior) · [Verify](#verification)

## What gets ejected

| Included | Excluded |
| --- | --- |
| Rust CLI/core, maintained language plugins and runtime templates | Dependency caches |
| Cargo manifests/lockfile and Go OpenAPI compiler source | Built output |
| Action/check sources, schemas, documentation and MIT license | Visual assets |

`--language` identifies the plugin to customize. The other plugins remain available because the CLI's profiles register them and depend on the common workspace. A rebuilt generator can still generate the other targets and compose plugin chains. `dotnet` selects the C# renderer.

This is source ejection: many renderers construct code directly in Rust, and others consume adjacent runtime template files. There is no separate template engine or flag that makes the installed binary read edited source files. Build and run your customized CLI to consume the edits.

## Build and generate

Install Rust and Go, then run from the ejected directory:

```sh
cargo build --locked -p poolster-cli
(cd openapi && go build -o ../target/debug/poolster-openapi .)
./target/debug/poolster generate /absolute/path/to/openapi.yaml \
  --language ruby --output /absolute/path/to/sdk
```

The first build can download Cargo and Go dependencies. The Go executable goes beside the rebuilt CLI. Alternatively, supply an existing compiler with `--openapi-compiler` or `POOLSTER_OPENAPI_BIN`.

For a release build, run `cargo build --locked --release -p poolster-cli` and place the compiler beside `target/release/poolster`. Existing generation configuration works with the rebuilt binary:

```sh
./target/debug/poolster generate --config /absolute/path/to/poolster.json
```

## Change generated behavior

1. Find your target's source directory in `EJECTED.md`; Ruby uses
   `crates/plugins/ruby/src/`.
2. Edit the renderer or an adjacent runtime file it consumes. For example,
   changing Ruby's `NOTICE` changes emitted source headers.
3. Rebuild the CLI and regenerate with the rebuilt binary.

For reusable extensions, use the [typed plugin interfaces](../../internals/typed-plugins.md). Add a Rust plugin crate, register its workspace/dependency entries, then integrate it with the profile and CLI configuration. Existing plugin `src/lib.rs` files provide maintained registration examples. Plugins can emit additional modules, integrate bundled author middleware and compose generation steps; source ejection preserves that architecture.

Keep your generator changes in version control. When updating Poolster, eject the new version into a fresh directory and compare source manifests before applying your changes. Retain `LICENSE` when distributing sources.

## Verification

Run the relevant plugin tests after customization and compile the resulting SDK with its native toolchain. Poolster includes an opt-in integration probe that ejects the workspace, edits a real Ruby renderer, rebuilds the CLI and Go compiler, then verifies generation consumed the edit:

```sh
cargo test -p poolster-cli eject::tests::rebuilt_ejected_renderer_consumes_custom_source \
  -- --ignored --nocapture
```

This probe needs cached Cargo dependencies because its nested build uses `--offline`. The regular bundle test validates file hashes, required build inputs and overwrite protection.
