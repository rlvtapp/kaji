# Command-line SDK generation

Kaji's CLI uses the same Rust plugins as the library. Its npm entry point is a
small Node launcher, not a JavaScript implementation of the generator. Platform
packages contain two executables: `kaji` (Rust) and `kaji-openapi` (the embedded Go
OpenAPI compiler). No Rust or Go installation is required for npm users.

The npm facade is published as `@relevate/kaji`. It chooses the matching
platform package and launches the native executable. Rust and Go are only
needed when building Kaji itself from source.

## Local source build

Run from the repository root (Rust and Go are required only for this source build):

```sh
cargo build -p kaji-cli
cd openapi
go build -o ../target/debug/kaji-openapi .
cd ..
./target/debug/kaji generate ./openapi.yaml --output ./generated --language go,typescript
```

On Windows, give the Go helper the `.exe` suffix. The CLI locates the helper next
to its own executable. Override that location with `--openapi-compiler <file>` or
`KAJI_OPENAPI_BIN`; an explicit flag takes precedence over the environment.

## Commands and options

```sh
npx @relevate/kaji init [--config <file>] [--input <openapi-file>] [--output <directory>]
npx @relevate/kaji generate                         # reads ./kaji.json
npx @relevate/kaji generate --config <file>
npx @relevate/kaji generate <openapi-file> --output <directory> --language <target>...
npx @relevate/kaji generate --artifacts <directory> --output <directory> --language <target>...
npx @relevate/kaji languages
npx @relevate/kaji --version
npx @relevate/kaji --help
```

| Option | Meaning | Default |
| --- | --- | --- |
| `--config` | JSON recipe to read; cannot be mixed with direct flags | `./kaji.json` when no direct flags are supplied |
| `--color` | Terminal color mode: `auto`, `always`, or `never` | `auto` (interactive terminals only; respects `NO_COLOR`) |
| `--output`, `-o` | Output root; target packages are placed beneath it | Required in direct mode |
| `--language`, `-l` | Comma-separated or repeated targets; `all` selects every SDK | Required |
| `--name` | API display name used for generation | `API` |
| `--sdk-version` | Generated package version | `0.1.0` |
| `--client-style` | `namespaced` or `flat`, shared by selected languages | `namespaced` |
| `--typescript-transport` | TypeScript HTTP client: `fetch` or `axios` | `fetch` |
| `--typescript-surface` | `client` (full SDK) or `raw` (operation functions) | `client` |
| `--typescript-client-name` | Explicit TypeScript client class name | Generator default |
| `--jobs` | Positive Go emission worker count; currently affects Go only | Bounded automatic selection |
| `--artifacts` | Already compiled Kaji OpenAPI artifact directory, instead of source | Unset |
| `--openapi-compiler` | Explicit Go compiler executable for source input | Bundled sibling executable |

Targets: `rust`, `typescript`, `go`, `python`, `php`,
`java`, `dotnet`, `elixir`. Each becomes a matching subdirectory, including when
only one target is selected. Advanced/custom plugin composition remains available
through the [Rust API](typed-plugins.md). The CLI does not load JavaScript
plugins; JSON names only select plugins built into the installed Kaji binary.

### JSON recipes and built-in plugins

`npx @relevate/kaji init` writes a non-destructive starter `kaji.json`; it never replaces an
existing file. Use `npx @relevate/kaji generate` to load that recipe. Paths inside the recipe
are relative to the config file, not the current terminal directory.

```json
{
  "openapi": { "input": "./openapi.yaml", "name": "Email", "version": "1.0.0" },
  "output": { "path": "./generated" },
  "defaults": { "client_style": "namespaced" },
  "packages": [
    {
      "language": "typescript", "path": "typescript", "name": "@acme/email",
      "plugins": [
        { "name": "sdk", "transport": "fetch", "client_name": "Email" },
        { "name": "zod" }, { "name": "tanstack-react-query" }, { "name": "msw" }
      ]
    },
    { "language": "go", "path": "go", "plugins": [{ "name": "sdk", "jobs": 4 }] },
    {
      "language": "artifacts", "path": "docs",
      "plugins": [{ "name": "redoc", "openapi_spec": "../openapi.yaml" }, { "name": "mcp" }]
    },
    { "language": "mock", "path": "mock-server", "plugins": [{ "name": "server", "port": 4010 }] }
  ]
}
```

`openapi` must provide exactly one of `input` (a YAML/JSON file) or `artifacts`
(a previous Kaji compiler artifact directory). `output.path` is required. Each
package has a safe relative `path`, a `language`, and a plugin array. `name` and
`client_style` are optional package overrides.

The full field-by-field schema, mode behavior, remote URL handling, and
TypeScript multi-client examples are in the [`kaji.json` reference](config-file.md).

| Package `language` | Built-in plugin names | Relevant plugin options |
| --- | --- | --- |
| `typescript` | `sdk`, `zod`, `tanstack-react-query`, `tanstack-vue-query`, `swr`, `faker`, `msw`, `cypress` | SDK: `transport` (`fetch`/`axios`), `surface` (`client`/`raw`), `client_name`, `group_by_tag`, `throw_on_error`. Artifacts: `output`, `clients_import`, `group_by_tag`. |
| `rust`, `go`, `python`, `php`, `java`, `dotnet`, `elixir` | `sdk` | Go SDK: `jobs`. |
| `mock` | `server` | `image`, `port`. |
| `artifacts` | `redoc`, `mcp` | `output`; ReDoc also accepts `openapi_spec`, `title`. |

`output` on an artifact plugin is a directory below that package's `path`.
`clients_import` defaults to `"./clients"`; set it if hook files must import
TypeScript operation functions from elsewhere. Read the [auxiliary generator
guide](auxiliary-generators.md) for framework dependencies and limits.

When Kaji also owns that TypeScript package through its `sdk` plugin, selecting
Zod, TanStack, SWR, Faker, or MSW adds the matching runtime dependency to its
`package.json`; selecting Cypress adds it as a development dependency. Cypress
project setup and framework peer dependencies remain the application's responsibility. Artifact-only TypeScript output intentionally
does not invent a package manifest.

`--language all` remains a direct-mode shortcut for the eight bundled SDK
languages; it uses the Fetch TypeScript transport. Use
`--typescript-transport axios` when direct mode needs Axios. JSON can declare
multiple TypeScript packages, each with its own `sdk.transport`. Direct mode
does not implicitly add auxiliary plugins or a mock package: JSON
declares exactly what a project generates.

Input can be a local YAML/JSON file or an `https://`/`http://` URL. Remote specs
are downloaded with a 120-second timeout and a 128 MiB size limit before the
bundled compiler runs. For example:

```sh
npx @relevate/kaji generate https://aka.ms/graph/v1.0/openapi.yaml \
  --output graph-sdk --language go --name "Microsoft Graph" --jobs 4
```

Go SDK source is always split into model and operation files; no layout toggle is needed.
The CLI reports compilation, SDK generation and writing durations separately.
Go worker counts are capped at 64; automatic selection uses at most 8 workers.
`--jobs` controls Go model/operation emission, not OpenAPI parsing or other languages.
`--artifacts` skips compilation for repeated language/configuration experiments.
Argument errors exit with code 2; compiler/generator/write errors exit with code 1.
Compiler failure does not write SDK output. Writes are not transactional if a
filesystem error occurs during materialization.

Generated files are overwritten. Custom starter files and unrelated files are
retained; stale generated files are not automatically deleted. Use a fresh output
directory when removing or renaming operations/models/targets, or changing
generation options that affect file paths. Removed files and target packages
remain until explicitly cleaned up.

## npm layout and release preparation

- `crates/kaji-cli`: native Rust command-line implementation.
- `packages/cli`: `@relevate/kaji` Node launcher with version-pinned optional native packages.
- `packages/cli/npm/<platform>`: generated platform package, containing both executables.
- `packages/npm`: optional unscoped `kaji` facade forwarding to `@relevate/kaji`.

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
scoped launcher version, optional dependency versions, and facade dependency.
Test the tarballs on their platforms, then publish the platform packages before
the scoped launcher. Publish the optional unscoped facade only if npm grants that
name; it is not needed to use `@relevate/kaji`.

For a local launcher smoke test, set `KAJI_BINARY` to the built Rust binary and
invoke `node packages/cli/bin/kaji.cjs --help`. Normal installed usage resolves the
matching optional package and verifies its version matches the launcher. Do not
install with `--omit=optional`; there is intentionally no postinstall downloader.
