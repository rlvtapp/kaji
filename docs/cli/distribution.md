# Distribute the Kaji CLI

[CLI reference](../cli.md) · [Source builds](../source-customization.md)

## npm layout and release preparation

- `crates/kaji-cli`: native Rust command-line implementation.
- `packages/cli`: `kajicli` Node launcher with version-pinned optional native packages.
- `packages/cli/npm/<platform>`: generated platform package, containing both executables.
- `packages/cli/sdk`: separate `@relevate/kaji` SDK and NAPI runtime.
- `packages/cli/sdk/npm/<platform>`: generated SDK addon and OpenAPI compiler.

```sh
node packages/cli/scripts/build-platform.mjs
```

An explicit platform argument can be `darwin-arm64`, `darwin-x64`,
`linux-x64-gnu`, or `win32-x64-msvc`. Cross-building Rust requires the matching
installed target and linker. The build script does not download targets, publish
packages, or create releases. Linux builds target glibc; musl and Linux ARM64
packages are not included in the initial matrix.

The manual **Build npm CLI packages** workflow builds and packs platform artifacts
for review without publishing them. Before a release, align the Cargo CLI version,
CLI and SDK package versions and their optional dependency versions.
Test the tarballs on their platforms, then publish the platform packages before
the CLI and SDK packages. The CLI package is `kajicli`; it installs the `kaji`
command. The SDK package is `@relevate/kaji`; import `@relevate/kaji/sdk`.

`@relevate/kaji@0.4.x` was a CLI package. Starting with the planned 0.5 npm
layout, CLI users must replace it with `kajicli`. The `@relevate/kaji` name is
reserved for the Node SDK and its addon. This is a breaking npm package change;
publish migration notes with the release. Neither package depends on the other.

For a local launcher smoke test, set `KAJI_BINARY` to the built Rust binary and
invoke `node packages/cli/bin/kaji.cjs --help`.
Normal installed usage resolves the
matching optional package and verifies its version matches the launcher.
Do not
install with `--omit=optional`; there is intentionally no postinstall downloader.

`ruby` emits a Ruby 3.1+ gem using the standard-library HTTP stack.
`swift`
emits a Swift 5.9+ Swift Package Manager library.
Each accepts
`base_url:`, `api_key:`, `bearer_token:`, and per-client `headers:`; the
namespaced surface adds resource facades without removing direct methods.
