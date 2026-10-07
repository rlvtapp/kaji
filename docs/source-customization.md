# Customize and rebuild the generator

Kaji supports both registered Rust plugins and edits to its maintained renderers. SDK authors can eject the sources from their installed CLI to customize what future SDKs contain. Customers continue to use the generated SDK; they do not install your generator workspace.

```sh
kaji eject --language ruby --out ./my-kaji
cd my-kaji
```

The destination must be new. Eject does not overwrite files or run generation, downloads, registry publication or GitHub installation. It writes a source workspace, `EJECTED.md` with target-specific entrypoints, and `EJECTED-SOURCES.json` with the original file SHA-256 hashes and generator version.

## What gets ejected

The bundle contains the native Rust CLI, normalized API/core interfaces, maintained language plugins, their runtime source templates, Cargo manifests and lockfile, the Go OpenAPI compiler sources, supporting action/check sources, schemas, documentation and the MIT license. It excludes dependency caches, built output and visual assets.

`--language` identifies the plugin to customize. The other plugins remain available because the CLI's profiles register them and depend on the common workspace. A rebuilt generator can still generate the other targets and compose plugin chains. `dotnet` selects the C# renderer.

This is source ejection: many renderers construct code directly in Rust, and others consume adjacent runtime template files. There is no separate template engine or flag that makes the installed binary read edited source files. Build and run your customized CLI to consume the edits.

## Build and generate

Install Rust and Go, then run from the ejected directory:

```sh
cargo build --locked -p kaji-cli
(cd openapi && go build -o ../target/debug/kaji-openapi .)
./target/debug/kaji generate /absolute/path/to/openapi.yaml \
  --language ruby --output /absolute/path/to/sdk
```

The first build can download Cargo and Go dependencies. The Go executable goes beside the rebuilt CLI. Alternatively, supply an existing compiler with `--openapi-compiler` or `KAJI_OPENAPI_BIN`.

For a release build, run `cargo build --locked --release -p kaji-cli` and place the compiler beside `target/release/kaji`. Existing generation configuration works with the rebuilt binary:

```sh
./target/debug/kaji generate --config /absolute/path/to/kaji.json
```

## Change generated behavior

For Ruby, edit `crates/plugins/ruby/src/`; the selected target's directory appears in `EJECTED.md`. For example, changing the Ruby renderer's `NOTICE` constant changes headers in the emitted Ruby source. Rebuild the CLI and regenerate to see that change. Editing a runtime file consumed by a renderer similarly changes the runtime bundled into future SDKs.

For reusable extensions, use the [typed plugin interfaces](typed-plugins.md). Add a Rust plugin crate, register its workspace/dependency entries, then integrate it with the profile and CLI configuration. Existing plugin `src/lib.rs` files provide maintained registration examples. Plugins can emit additional modules, integrate bundled author middleware and compose generation steps; source ejection preserves that architecture.

Keep your generator changes in version control. When updating Kaji, eject the new version into a fresh directory and compare source manifests before applying your changes. Retain `LICENSE` when distributing sources.

## Verification

Run the relevant plugin tests after customization and compile the resulting SDK with its native toolchain. Kaji includes an opt-in integration probe that ejects the workspace, edits a real Ruby renderer, rebuilds the CLI and Go compiler, then verifies generation consumed the edit:

```sh
cargo test -p kaji-cli eject::tests::rebuilt_ejected_renderer_consumes_custom_source \
  -- --ignored --nocapture
```

This probe needs cached Cargo dependencies because its nested build uses `--offline`. The regular bundle test validates file hashes, required build inputs and overwrite protection.
