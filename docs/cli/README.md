# Poolster from the terminal

Keep a `poolster.json` recipe. Generate, review, repeat.

[Quickstart](quickstart.md) · [Configure packages](config.md) ·
[Recipes](recipes.md) · [Commands](commands.md)

## Generate your first package

```sh
npx poolster init --input ./openapi.yaml --output ./generated
npx poolster generate --config poolster.json
npx poolster generate --config poolster.json --check
```

The last command checks for drift without writing output. Build and test the
generated package separately with its native tools.

Prefer pip? Install `poolster-cli` and use `poolster` in place of `npx poolster`.
Both launchers bundle the generator and OpenAPI compiler.
[Installation and source builds →](reference.md)

## Shape the result

| Task | Guide |
| --- | --- |
| Choose languages, packages and helpers | [Recipe configuration](config.md) |
| Generate several packages at once | [Multi-package recipes](recipes.md) |
| Load a private contract | [Remote input recipe](recipes.md#private-remote-contract) |
| Bundle middleware or custom source | [SDK customization](../reference/regeneration/sdk-customization.md) |
| Protect edits during regeneration | [Safe regeneration](../reference/regeneration/safe-regeneration.md) |
| Generate GraphQL collections or executable CLIs | [GraphQL tools](../reference/outputs/graphql-tools.md) |
| Inspect native non-OpenAPI contracts | [Input plugins](../reference/inputs/input-plugins.md) |

The HTTP SDK workflow accepts Swagger 2.0 and OpenAPI 3.0/3.1/3.2.
New input providers need a source build and compatible output consumers.

## Check your work

```sh
poolster generate --config poolster.json --check --format json
```

The report lists added, modified and removed paths. Drift makes the command fail.
A clean report proves repeatable generation; use [SDK tests](../guides/testing.md)
to check API behavior.

## Ship it

[Prepare SDK PRs](../reference/automation/sdk-automation.md) → [Release and publish](../reference/automation/sdk-publishing.md)

Repository and registry setup belongs in those guides. Use
[CI integration](../reference/automation/ci-integration.md) for a simple generation check.

## Need a custom plugin?

Use the [Rust library](../rust/README.md) to compose native plugins.
The CLI recipe selects its bundled plugin set.

[Complete CLI reference →](reference.md)

## Deliver and customize SDKs

- [GitHub Actions and generated PRs](automation.md)
- [Release and publish an SDK](../reference/automation/sdk-publishing.md)
- [Customize output and eject the generator](customization.md)

[Inspect the generation plan](plan.md) — plugins, contracts and selected providers.
