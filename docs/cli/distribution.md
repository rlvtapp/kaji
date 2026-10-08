# Distribute the Poolster CLI

[CLI reference](../cli.md) · [Source builds](../source-customization.md)

## npm layout and release preparation

- `crates/cli`: native Rust command-line implementation.
- `packages/npm/cli`: `poolster` Node launcher with version-pinned optional native packages.
- `packages/npm/platform/cli/<platform>`: generated platform package, containing both executables.
- `packages/npm/sdk`: separate `@relevate/poolster` SDK and NAPI runtime.
- `packages/npm/platform/node/<platform>`: generated SDK addon and OpenAPI compiler.

```sh
node packages/npm/cli/scripts/build-platform.mjs
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
the CLI and SDK packages. The CLI package is `poolster`; it installs the `poolster`
command. The SDK package is `@relevate/poolster`; import `@relevate/poolster`.

`@relevate/poolster@0.4.x` was a CLI package. Starting with the planned 0.5 npm
layout, CLI users must replace it with `poolster`. The `@relevate/poolster` name is
reserved for the Node SDK and its addon. This is a breaking npm package change;
publish migration notes with the release. Neither package depends on the other.

For a local launcher smoke test, set `POOLSTER_BINARY` to the built Rust binary and
invoke `node packages/npm/cli/bin/poolster.cjs --help`.
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
