# Commands and flags

## Everyday commands

```sh
kaji init [--config <file>] [--input <file-or-url>] [--output <directory>]
kaji generate                              # reads ./kaji.json
kaji generate --config <file>
kaji generate <file-or-url> --output <directory> --language <target>
kaji discover <query> [--limit <count>] [--format human|json]
kaji download <api-id> --output <file> [--version <version>]
kaji languages
kaji --version
```

Use `npx @relevate/kaji` in place of `kaji` when Kaji is not installed in your
project. The npm facade launches the native generator.

`discover` searches the public APIs.guru OpenAPI directory; `download` writes a
preferred (or explicitly selected) directory version to a new local file. See
[OpenAPI discovery and download](../discovery.md) for examples and safeguards.

## Direct-mode example

```sh
npx @relevate/kaji generate openapi.yaml \
  --output ./generated \
  --language typescript,go \
  --name "Pet Store" \
  --sdk-version 1.0.0
```

For a focused client from a large contract, repeat `--include-path` and
`--exclude-path`. Includes are ORed and excludes win:

```sh
kaji generate openapi.yaml --output ./generated --language typescript \
  --include-path '/messages*' --include-path '/admin/users*' \
  --exclude-path '/admin/users/audit*'
```

The same selectors belong in `openapi.paths` in `kaji.json` for committed
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

Each successful run also writes `.kaji/generation.lock.json` in the output.
Commit this secret-free generation metadata with the generated files; it pins
the Kaji version, contract/artifact hashes, path selection, targets, and
selected operation list used for that output.

Targets are `rust`, `typescript`, `go`, `python`, `php`, `java`, `csharp`, and
`elixir`. Configuration errors exit with `2`; compiler, generation, and write
errors exit with `1`.

`csharp` generates a .NET 8 C# package. `dotnet` remains accepted as a legacy
alias for existing scripts.

See [the complete CLI reference](../cli.md) for every option and source-build
instructions.
