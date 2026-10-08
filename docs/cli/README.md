# Kaji from the terminal

Keep a `kaji.json` recipe. Generate, review, repeat.

[Quickstart](quickstart.md) · [Configure packages](config.md) ·
[Recipes](recipes.md) · [Commands](commands.md)

## Generate your first package

```sh
npx kajicli init --input ./openapi.yaml --output ./generated
npx kajicli generate --config kaji.json
npx kajicli generate --config kaji.json --check
```

The last command checks for drift without writing output. Build and test the
generated package separately with its native tools.

Prefer pip? Install `kaji-cli` and use `kaji` in place of `npx kajicli`.
Both launchers bundle the generator and OpenAPI compiler.
[Installation and source builds →](../cli.md)

## Shape the result

| Task | Guide |
| --- | --- |
| Choose languages, packages and helpers | [Recipe configuration](config.md) |
| Generate several packages at once | [Multi-package recipes](recipes.md) |
| Load a private contract | [Remote input recipe](recipes.md#private-remote-contract) |
| Bundle middleware or custom source | [SDK customization](../sdk-customization.md) |
| Protect edits during regeneration | [Safe regeneration](../safe-regeneration.md) |
| Inspect native non-OpenAPI contracts | [Input plugins](../input-plugins.md) |

The HTTP SDK workflow accepts Swagger 2.0 and OpenAPI 3.0/3.1/3.2.
New input providers need a source build and compatible output consumers.

## Check your work

```sh
kaji generate --config kaji.json --check --format json
```

The report lists added, modified and removed paths. Drift makes the command fail.
A clean report proves repeatable generation; use [SDK tests](../guides/testing.md)
to check API behavior.

## Ship it

[Prepare SDK PRs](../sdk-automation.md) → [Release and publish](../sdk-publishing.md)

Repository and registry setup belongs in those guides. Use
[CI integration](../ci-integration.md) for a simple generation check.

## Need a custom plugin?

Use the [Rust library](../library/README.md) to compose native plugins.
The CLI recipe selects its bundled plugin set.

[Complete CLI reference →](../cli.md)
