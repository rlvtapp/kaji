# Generate SDK pull requests with GitHub Actions

← [CLI](README.md)

Choose the automation you need:

| Task | Path |
| --- | --- |
| Generate or check files during CI | Run the Node SDK or the repository action |
| Open a generated SDK PR | CLI delivery commands and generated workflows |
| Build and publish a released SDK | Delivery metadata, release workflow and registry trust |

## Generate with the repository action

Use a `poolster.json` recipe with the repository action:

```yaml
name: Generate SDK
on: [push, pull_request]
permissions:
  contents: read
jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rlvtapp/poolster/packages/integrations/github@main
        with:
          config: poolster.json
```

Pin the action ref and its `version` input to tested releases in production.
See the [action inputs](../../packages/integrations/github/README.md) and
[drift checks](checks.md). Running generation does not itself open a PR.

## Open generated PRs

Poolster's SDK delivery scaffolding uses the CLI and a `poolster.json` recipe.
The Node `generate()` API generates packages; it does not create a GitHub PR.
Follow the [delivery recipe](../reference/automation/sdk-automation.md#1-declare-the-package-and-its-delivery-metadata)
to describe package build, test and publisher commands, then:

```sh
poolster generate --config poolster.json
poolster sdk init --root generated --config poolster.json --auth app --dry-run
poolster sdk pr --config poolster.json --repository acme/sdk-repo --dry-run
```

Replace the repository and configure the authentication method you selected.
Previewing shows the planned work. After reviewing it, run the command without
`--dry-run` to write the scaffold or open the PR. Choose the same-repository or
separate SDK repository layout before installing the generated workflows.

The complete [SDK automation guide](../reference/automation/sdk-automation.md)
covers destination setup, GitHub App, broker or token authentication, version
bumps and branch updates. Scaffolding local files does not create repositories,
secrets, Apps or trusted publishers.

## Publish the reviewed SDK

Merge the generated content PR through your normal checks, then use the release
workflow for its release tag. Configure npm trusted publishing and package
ownership before the first upload. Publication runs for the checked tag, with
registry-specific verification and retry behavior.

[Release and publishing](../reference/automation/sdk-publishing.md) explains the
exact release boundary. These are SDK delivery capabilities, separate from
publishing Poolster itself. Pin the action and launcher to versions that contain
the commands you use; this checkout includes unreleased features.

**More:** [Repository action](../../packages/integrations/github/README.md) ·
[Periodic specification fetching](../reference/automation/github-actions.md#fetch-a-remote-specification-periodically)
