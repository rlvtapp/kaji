# Generate and deliver SDKs with the CLI

Use the CLI when you own an SDK generated from a contract and want generation to run in a repository, terminal, or CI job. A committed `kaji.json` records the input, output, packages, plugins, and author customization. Each rerun produces the same selected artifacts for review.

Kaji accepts Swagger 2.0 and OpenAPI 3.0/3.1 YAML or JSON. Start with a local document and a writable output directory. The installed generator bundles its OpenAPI compiler; you need native language tooling only when building/testing the generated packages. Source builds have separate Rust/Go prerequisites described in the [CLI reference](../cli.md).

## Start with a local contract

For the smallest complete walkthrough, use the [quickstart](quickstart.md). It uses the repository's Notes contract and checks the generated tree before requiring any target dependency installation.

For your own document:

```sh
npx @relevate/kaji init --input ./openapi.yaml --output ./generated
npx @relevate/kaji generate --config kaji.json
npx @relevate/kaji generate --config kaji.json --check
```

`init` creates a reviewable recipe without overwriting an existing one. The last command compares expected output with the destination without writing it. A successful check means the output matches generation; it does not mean the SDK has passed its native build or API tests.

If you prefer a Python-installed launcher:

```sh
python -m pip install kaji-cli
kaji init --input ./openapi.yaml --output ./generated
kaji generate --config kaji.json
```

The pip distribution includes the native generator/compiler and does not need Node, Rust, or Go to generate. Use a distributed version containing the features you select; the source tree can contain commands that have not yet reached a published launcher.

## Build the author's repeatable workflow

**Choose the output.** Read [recipe configuration](config.md) to set names, package paths, target languages, plugins, and private source references. [Recipes](recipes.md) covers several SDKs from one contract and reusable compiler artifacts. For a one-off experiment, direct mode is shorter:

```sh
kaji generate openapi.yaml --output generated --language go
```

**Bundle your behavior.** [SDK customization](../sdk-customization.md) explains package-scoped middleware and source overlays. Keep their source outside the generated destination. Bundled middleware is enabled in the delivered SDK; consumers do not have to register it themselves. Build and exercise that source with the package's native tools.

**Review and check.** [Safe regeneration](../safe-regeneration.md) explains ownership, edited-file conflicts, create-once files, and obsolete generated files. Commit the recipe, customization sources, and generated bookkeeping with your chosen output policy. Run native build/tests separately; use [testing generated SDKs](../guides/testing.md) and [verification](../verification.md) to choose meaningful checks.

```sh
kaji generate --config kaji.json --check --format json
```

JSON output reports added, modified, and removed paths. A nonempty drift report fails the check. Restore accidental edits or move intended changes into the author customization source before regenerating.

## Continue to SDK repository PRs and publication

[SDK automation](../sdk-automation.md) begins where generation ends: declare package build/test/release metadata, prepare editable workflows, and synchronize generated changes through an SDK PR. It covers the same repository and a separate SDK repository, independent versions, source authentication, and Release Please.

[SDK publishing](../sdk-publishing.md) then covers the exact tagged package, registry identity, trusted publishing, protected environments, and custom publisher commands. A generated PR and a release PR are separate reviews. Local `sdk init`/`sdk sync` setup does not create registry accounts, configure GitHub installations, or publish a package. Read the full setup guide before running its mutation commands.

For basic CI drift checks, use [CI integration](../ci-integration.md). For GitHub App authentication, use [App setup](../github-app.md) and the [broker guide](../github-app-broker.md). Forked/unpublished launchers and editable delivery actions are covered by [source customization](../source-customization.md).

## Reference and next steps

[Commands and flags](commands.md) is the everyday reference; [complete CLI reference](../cli.md) includes source installation and the exhaustive options. [Generated SDKs](../generated-sdks.md) describes what customers receive, including language-specific limits. [Examples](../../examples/README.md) provides complete recipes rather than isolated fragments.

If you need a native custom plugin, use the [Rust library workflow](../library/README.md). CLI configuration selects bundled plugins and does not load arbitrary Rust or JavaScript plugin code.
