# Command-line SDK generation

[CLI workflow](README.md) · [Everyday commands](commands.md) · [Contract checks](checks.md)

> Prefer the task-focused [CLI documentation](README.md) for a first
> project: [quickstart](quickstart.md), [`poolster.json` recipes](config.md),
> [commands](commands.md), and [common recipes](recipes.md). This page
> remains the exhaustive reference and source-build guide.

The CLI runs Poolster's Rust plugins. The `poolster` npm launcher selects the
platform package containing `poolster` and the Go `poolster-openapi` compiler.

Installed users do not need Rust or Go. Source builds do.

## Local source build

Run from the repository root (Rust and Go are required only for this source build):

```sh
cargo build -p poolster-cli
cd openapi
go build -o ../target/debug/poolster-openapi .
cd ..
./target/debug/poolster generate ./openapi.yaml --output ./generated --language go,typescript
```

On Windows, give the Go helper the `.exe` suffix. The CLI locates the helper next
to its own executable. Override that location with `--openapi-compiler <file>` or
`POOLSTER_OPENAPI_BIN`; an explicit flag takes precedence over the environment.

## Commands and options

```sh
npx poolster init [--config <file>] [--input <openapi-file>] [--output <directory>]
npx poolster generate                         # reads ./poolster.json
npx poolster generate --config <file>
npx poolster generate <openapi-file> --output <directory> --language <target>...
npx poolster generate --artifacts <directory> --output <directory> --language <target>...
npx poolster mock serve <openapi-file> [--port <port>]
npx poolster check <openapi-file> [--format human|json]
npx poolster show <openapi-file> [--include-path <pattern>] [--exclude-path <pattern>]
npx poolster update [--output <directory>] [--force]
npx poolster auth <login|logout|status> ...
npx poolster eject --language ruby --out ./my-poolster
npx poolster languages
npx poolster --version
npx poolster --help
```

`eject` exports a rebuildable generator workspace into a new directory. Edit the
renderers or add plugins, then rebuild your CLI; see [source customization](../reference/regeneration/source-customization.md).

| Option | Meaning | Default |
| --- | --- | --- |
| `--config` | JSON recipe to read; cannot be mixed with direct flags | `./poolster.json` when no direct flags are supplied |
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
| `--artifacts` | Already compiled Poolster OpenAPI artifact directory, instead of source | Unset |
| `--openapi-compiler` | Explicit Go compiler executable for source input | Bundled sibling executable |
| `--include-path` | Repeatable OpenAPI path glob to include | Every path |
| `--exclude-path` | Repeatable OpenAPI path glob to omit after inclusion | None |

Targets: `rust`, `rust-cli`, `typescript`, `typescript-cli`, `go`, `python`, `php`, `symfony`,
`java`, `csharp`, `elixir`, `ruby`, `swift`. Each becomes a matching subdirectory, including when
only one target is selected. Advanced/custom plugin composition remains available
through the [Rust API](../internals/typed-plugins.md). The CLI does not load JavaScript
plugins; JSON names only select plugins built into the installed Poolster binary.

`csharp` emits a .NET 8 C# SDK. `dotnet` is a backwards-compatible selector
for existing commands and recipes.

Path globs must start with `/`; `*` matches any sequence (including `/`) and
`?` one character. Includes are ORed and excludes take precedence. The same
selection is available as `openapi.paths.include` / `openapi.paths.exclude` in
`poolster.json`. A selection with no remaining operations fails before output is
written.

Every successful generation emits `.poolster/generation.lock.json` below the
output root. Commit it with generated files: it records Poolster's version,
secret-free input/config/artifact hashes, selected paths, targets, and
operations so a regeneration is reviewable and repeatable.

### JSON recipes and built-in plugins

`npx poolster init` writes a non-destructive starter `poolster.json`; it never replaces an
existing file. Use `npx poolster generate` to load that recipe. Paths inside the recipe
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
(a previous Poolster compiler artifact directory). `output.path` is required. Each
package has a safe relative `path`, a `language`, and a plugin array. `name` and
`client_style` are optional package overrides.

The full field-by-field schema, mode behavior, remote URL handling, and
TypeScript multi-client examples are in the [`poolster.json` reference](../reference/configuration/config-file.md).

| Package `language` | Built-in plugin names | Relevant plugin options |
| --- | --- | --- |
| `typescript` | `sdk`, `zod`, `tanstack-react-query`, `tanstack-vue-query`, `swr`, `faker`, `msw`, `cypress` | SDK: `transport` (`fetch`/`axios`), `surface` (`client`/`raw`), `client_name`, `group_by_tag`, `throw_on_error`. Artifacts: `output`, `group_by_tag`; Zod/Faker/MSW/Cypress: `max_file_bytes`; query consumers: `max_operations_per_file`, `uses.operations`. |
| `rust`, `go`, `python`, `php`, `java`, `csharp`, `elixir`, `ruby`, `swift` | `sdk` | Go SDK: `jobs`. |
| `symfony` | `sdk` | `sdk_package` to reference the generated PHP SDK Composer package. |
| `mock` | `server` | `image`, `port`. |
| `artifacts` | `redoc`, `mcp` | `output`; ReDoc also accepts `openapi_spec`, `title`. |

`output` on an artifact plugin is a directory below that package's `path`.
For direct standalone artifact renderers, `clients_import` defaults to
`"./clients"`; set it if hook files must import
TypeScript operation functions from elsewhere. Read the [auxiliary generator
guide](../reference/outputs/auxiliary-generators.md) for framework dependencies and limits.

When Poolster also owns that TypeScript package through its `sdk` plugin, selecting
Zod, TanStack, SWR, Faker, or MSW adds the matching runtime dependency to its
`package.json`; selecting Cypress adds it as a development dependency. Cypress
project setup and framework peer dependencies remain the application's responsibility. Artifact-only TypeScript output intentionally
does not invent a package manifest.

`--language all` remains a direct-mode shortcut for every bundled SDK
languages; it uses the Fetch TypeScript transport. Use
`--typescript-transport axios` when direct mode needs Axios. JSON can declare
multiple TypeScript packages, each with its own `sdk.transport`. Direct mode
does not implicitly add auxiliary plugins or a mock package: JSON
declares exactly what a project generates.

Input can be a local YAML/JSON file or an `https://`/`http://` URL. Remote specs
are downloaded with a 120-second timeout and a 128 MiB size limit before the
bundled compiler runs. For example:

```sh
npx poolster generate https://aka.ms/graph/v1.0/openapi.yaml \
  --output graph-sdk --language go --name "Microsoft Graph" --jobs 4
```

Go SDK source is always split into model and operation files; no layout toggle is needed.
The CLI reports compilation, SDK generation and writing durations separately.
Go worker counts are capped at 64; automatic selection uses at most 8 workers.
`--jobs` controls Go model/operation emission, not OpenAPI parsing or other languages.

`--artifacts` skips compilation for repeated language/configuration experiments.

| Result | Exit code |
| --- | --- |
| Invalid arguments | `2` |
| Compiler, generator or write failure | `1` |

Compiler failure leaves SDK output untouched. Materialization can be interrupted
by a filesystem error.

### Contract checks

Validate operation IDs, success responses and path bindings before generation.
[Check rules, severity and baselines →](checks.md)

Use [safe regeneration](../reference/regeneration/safe-regeneration.md) when applying output changes.
Poolster protects edited owned files and removes unchanged stale owned files.
Writing can still be interrupted by a filesystem error.

## npm layout and release preparation

Maintain platform packages and the npm launcher together.
[Distribution and release checklist →](distribution.md)
