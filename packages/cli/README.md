# @relevate/kaji

Thin Node launcher for Kaji's native SDK generator. SDK generation runs in Rust;
OpenAPI parsing runs in Kaji's bundled Go compiler. Node only selects and launches
the correct executable. End users do not need Rust or Go installed.

This directory prepares the npm distribution; the package and native platform
artifacts must be built and published before the install command below works.
No npm release is implied by this source tree.

After publication:

```sh
npm install --save-dev @relevate/kaji
npx @relevate/kaji generate ./openapi.yaml --output ./sdk --language go,typescript
npx @relevate/kaji languages
npx @relevate/kaji --help
```

The generator accepts a local OpenAPI file or `--artifacts` directory, not a URL.
Download remote specifications first, preserving local files referenced by `$ref`.
Each selected target has its own output directory. Generated files are overwritten;
custom starter files and unrelated files are retained. Use a clean output directory
after removing or renaming operations/models/targets or changing file paths,
because stale generated files are not deleted automatically. Go source is always
split into model and operation files.

Supported initial binary packages: macOS ARM64/x64, Linux x64 with glibc,
Windows x64. Linux ARM64 and Alpine/musl are not packaged yet. Optional dependencies
must be enabled. There are no install scripts or runtime binary downloads.

## Development

From the repository root:

```sh
node packages/cli/scripts/build-platform.mjs
KAJI_BINARY="$PWD/packages/cli/npm/darwin-arm64/kaji" node packages/cli/bin/kaji.cjs --help
node --test packages/cli/test/*.test.cjs
```

Use your platform directory in the second command. The build script needs Rust,
the selected Rust target, an appropriate linker and Go. Both native binaries are
assembled under `packages/cli/npm/<platform>/`; source code and build tools are
not shipped to users. `KAJI_BINARY` is an optional local development override.
`KAJI_OPENAPI_BIN` overrides the Go helper for source builds.

Full CLI and release instructions: [docs/cli.md](https://github.com/rlvtapp/kaji/blob/main/docs/cli.md).
