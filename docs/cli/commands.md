# Commands and flags

## Everyday commands

```sh
poolster init [--config <file>] [--input <file-or-url>] [--output <directory>]
poolster generate                              # reads ./poolster.json
poolster generate --config <file>
poolster generate <file-or-url> --output <directory> --language <target>
poolster show <openapi-file> [--include-path <pattern>] [--exclude-path <pattern>]
poolster update [--output <directory>] [--force]
poolster auth <login|logout|status> ...
poolster discover <query> [--limit <count>] [--format human|json]
poolster download <api-id> --output <file> [--version <version>]
poolster contract plugins [--format human|json]
poolster contract inspect <file> --input-format <format> [--provider <id>]
poolster languages
poolster --version
```

Use `npx poolster` in place of `poolster` when Poolster is not installed in your
project. The npm facade launches the native generator.

`discover` searches the public APIs.guru OpenAPI directory; `download` writes a
preferred (or explicitly selected) directory version to a new local file. See
[OpenAPI discovery and download](../discovery.md) for examples and safeguards.

## Direct-mode example

```sh
npx poolster generate openapi.yaml \
  --output ./generated \
  --language typescript,go \
  --name "Pet Store" \
  --sdk-version 1.0.0
```

For a focused client from a large contract, repeat `--include-path` and
`--exclude-path`. Includes are ORed and excludes win:

```sh
poolster generate openapi.yaml --output ./generated --language typescript \
  --include-path '/messages*' --include-path '/admin/users*' \
  --exclude-path '/admin/users/audit*'
```

The same selectors belong in `openapi.paths` in `poolster.json` for committed
workflows. They use `*` (any sequence, including `/`) and `?` (one character),
must start with `/`, and fail if no operation remains.

| Flag | Meaning |
| --- | --- |
| `--output`, `-o` | Required direct-mode output root. |
| `--language`, `-l` | Comma-separated/repeated targets, or `all`. |
| `--name` / `--sdk-version` | Generated display name and version. |
| `--client-style` | `namespaced` (default) or `flat`. |
| `--typescript-transport` | `fetch` (default) or `axios`. |
| `--typescript-surface` | `client` (default) or raw operation functions. |
| `--jobs` | Bounded Go rendering worker count. |
| `--artifacts` | Use prior compiler artifacts instead of source input. |
| `--include-path` | Repeatable OpenAPI path pattern to include. |
| `--exclude-path` | Repeatable OpenAPI path pattern to exclude after inclusion. |
| `--color` | `auto`, `always`, or `never`. |

Each successful run also writes `.poolster/generation.lock.json` in the output.
Commit this secret-free generation metadata with the generated files; it pins
the Poolster version, contract/artifact hashes, path selection, targets, and
selected operation list used for that output.

Targets are `rust`, `typescript`, `go`, `python`, `php`, `java`, `csharp`,
`elixir`, `ruby`, and `swift`. Configuration errors exit with `2`; compiler,
generation, and write errors exit with `1`.

`csharp` generates a .NET 8 C# package. `dotnet` remains accepted as a legacy
alias for existing scripts.

`ruby` generates a Ruby 3.1+ gem with the standard library; `swift` generates
a Swift Package Manager library targeting Swift 5.9+.

## Inspect and update

`show` compiles a local contract and prints the same path slice Poolster would
generate. Use `--format json` when an agent needs the selected operations in a
machine-readable form:

```sh
poolster show openapi.yaml --include-path '/messages*' --format json
```

`update --output generated` finds direct-generation lock files below the root.
It replays a lock when its local source or artifacts changed, skips unchanged
local inputs, and always re-fetches public URL inputs. Config-generated output
has an explicit recipe already, so rerun `poolster generate --config poolster.json`.
Use `--force` to regenerate every replayable lock.

## Private-spec auth profiles

Poolster auth profiles store only an environment-variable name, never a token.
They make a named provider token reusable in authenticated remote inputs:

```sh
export GITHUB_TOKEN=…
poolster auth login github --token-env GITHUB_TOKEN
poolster auth status
```

Reference that profile in `poolster.json` with
`"token": { "profile": "github" }`, then use `poolster auth logout github` to
remove the mapping. `KAJI_CONFIG_HOME` overrides the profile-store directory.

See [the complete CLI reference](../cli.md) for every option and source-build
instructions.

## Inspect other input formats

Use a source build to inspect native contracts:

```sh
poolster contract plugins --format json
poolster contract inspect schema.graphql --input-format graphql --format json
```

[Input providers and format support →](../input-plugins.md)

## Check a contract

```sh
poolster check openapi.yaml --format json
```

[Rules, severity and baselines →](checks.md)
