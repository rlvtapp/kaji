# Commands and flags

## Everyday commands

```sh
kaji init [--config <file>] [--input <file-or-url>] [--output <directory>]
kaji generate                              # reads ./kaji.json
kaji generate --config <file>
kaji generate <file-or-url> --output <directory> --language <target>
kaji languages
kaji --version
```

Use `npx @relevate/kaji` in place of `kaji` when Kaji is not installed in your
project. The npm facade launches the native generator.

## Direct-mode example

```sh
npx @relevate/kaji generate openapi.yaml \
  --output ./generated \
  --language typescript,go \
  --name "Pet Store" \
  --sdk-version 1.0.0
```

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
| `--color` | `auto`, `always`, or `never`. |

Targets are `rust`, `typescript`, `go`, `python`, `php`, `java`, `dotnet`, and
`elixir`. Configuration errors exit with `2`; compiler, generation, and write
errors exit with `1`.

See [the complete CLI reference](../cli.md) for every option and source-build
instructions.
