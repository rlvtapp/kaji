# Generate with the CLI

The CLI is the best route when Kaji runs in a repository, terminal, script, or
CI job. It accepts Swagger 2.0 and OpenAPI 3.0/3.1 YAML or JSON, compiles the
contract, and writes one or more SDK packages.

```sh
npx @relevate/kaji init --input ./openapi.yaml --output ./generated
npx @relevate/kaji generate
```

`init` makes a reviewable `kaji.json`; `generate` repeats it exactly. Use that
config-first route for projects you expect to regenerate. Direct flags remain
useful for one-off experiments.

## Learn in order

1. [Quickstart](quickstart.md) — generate and consume a first SDK.
2. [`kaji.json` recipes](config.md) — sources, packages, plugins, and secrets.
3. [Commands and flags](commands.md) — direct mode, output behavior, exit codes.
4. [Recipes](recipes.md) — multiple SDKs, private specs, and compiler artifacts.
5. [CI integration](../ci-integration.md) — keep outputs in sync automatically.

| Mode | Best for | Example |
| --- | --- | --- |
| Config-first | Repeated generation, helpers, mocks, several packages | `kaji generate --config kaji.json` |
| Direct | A single SDK or experiment | `kaji generate openapi.yaml --output generated --language go` |

For source builds and the exhaustive option table, see the
[complete CLI reference](../cli.md).
