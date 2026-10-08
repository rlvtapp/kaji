# GitHub Actions for SDK authors

Choose where your specification, generation and SDK releases live.
This page covers repository handoffs, specification relay and scheduled fetching.
For the full SDK PR → release PR → exact-tag publication flow, see
[SDK automation](sdk-automation.md).

> Use a Kaji launcher containing the commands/options shown here. The current
> source checkout and a previously published launcher/action can differ; verify
> the version you install in CI before committing its scaffold.

**On this page:** [Layout](#choose-where-generation-runs) · [Bootstrap](#bootstrap-generation-and-release-in-one-repository) · [Per language](#deliver-each-language-to-its-own-repository) · [Relay](#relay-a-specification-to-an-sdk-repository) · [Schedule](#fetch-a-remote-specification-periodically) · [Status](#inspect-local-and-remote-automation-state)

## Choose where generation runs

| Your situation | Workflow arrangement | Where the recipe lives |
| --- | --- | --- |
| API and SDK code share a repository | `sdk init` generates local SDK/check/release workflows. | Same repository as the specification. |
| The API repository owns generation; SDK output lives elsewhere | `sdk init --repository OWNER/SDK` generates the source workflow and stages destination setup. | API repository; generated code is sent to the SDK repository. |
| The API repository owns generation; each language has its own repository | `sdk init --repository-pattern 'OWNER/api-{lang}'` writes source jobs per language and stages setup per destination. | API repository; each job delivers only its selected language. |
| The API repository only exports a specification; SDK authors own generation | `sdk connect` relays the spec as a destination PR; destination `sdk init` handles generation and release. | SDK repository, pointing at the relayed specification path. |

Choose one generation owner per output branch. That repository owns the recipe,
package options and author source. A relay copies only the specification;
generation and release remain in the destination.

## Bootstrap generation and release in one repository

First generate and verify your packages locally:

```sh
kaji generate --config kaji.json
kaji generate --config kaji.json --check --format json
kaji sdk list --root generated --json
kaji sdk init --root generated --config kaji.json --auth app --dry-run
```

Inspect the planned files, then rerun without `--dry-run` and with a Kaji launcher
version that actually contains these commands.
Review and commit the generated
workflows/actions along with the recipe and author sources.

`--base` defaults to
`main`; for `sdk init`/`sync` it selects the source push trigger, SDK PR base and
Release Please target branch.
Use `--base your-branch` for another convention.
Configure the App,
registry trust and protected `release` environment separately.
The
[bootstrap guide](sdk-automation.md#3-preview-the-workflow-bootstrap) covers each
file's role.

For generation in an API repository and output in another repository, add
`--repository OWNER/SDK`. Destination actions/check/release files are staged under
`.kaji/sdk-repository-setup/`; its contents must be copied and committed in the
SDK repository first. This is a different arrangement from specification relay.

## Deliver each language to its own repository

```sh
kaji sdk init --root generated --config kaji.json \
  --repository-pattern 'acme/api-{lang}' --auth app --base main --dry-run
```

The pattern requires exactly one `{lang}` and cannot accompany `--repository`.
Rerun without `--dry-run` after review. Use the same flag with `sdk sync` when the
package list changes; `--schedule`, `--bump`, `--actions` and authentication options
still apply. Configuration-based per-package routing is not available.

Each language job uses a destination-scoped token and `sdk pr --language LANG`.
Packages sharing a language share a repository; output paths stay intact.
Commit the recipe, author sources and source workflow in the API repository.

Each destination has staged setup under
`.kaji/sdk-repository-setup/OWNER/REPO/`: CI/release workflows, check/publish
helpers and Release Please configuration.

1. Copy that directory's **contents** into the matching repository root.
2. Review and commit with owner/workflow authority before the first SDK PR.
3. Configure registry trust separately in each destination.

Routine App tokens have Contents/Pull requests access; installing workflow files
requires bootstrap authority. The command creates no remote repository, hosted
service or App installation.

To preview a bootstrap PR instead of copying manually:

```sh
kaji sdk install --setup .kaji/sdk-repository-setup/acme/api-typescript \
  --repository acme/api-typescript --dry-run
```

Rerun without `--dry-run` to open the PR using credentials authorized to write
workflow files. Commit the API repository's source workflow separately.
Tracked installed files preserve edits and report refresh conflicts. See the
[complete bootstrap procedure](sdk-automation.md#repository-per-language).

## Relay a specification to an SDK repository

Choose this arrangement when an API team owns its contract but SDK authors want
to own the generator recipe, customer policy and release lifecycle elsewhere.
The intended review sequence is:

```text
API contract changes
  -> relay opens/updates specification PR in SDK repository
  -> owner reviews and merges specification PR
  -> destination generation workflow opens generated SDK content PR
  -> owner merges SDK content PR
  -> Release Please version/changelog PR
  -> owner merges release PR
  -> exact tag checks and publication
```

The specification PR and generated SDK PR are separate review boundaries. A relay
PR does not publish a package or automatically approve a new API contract.

### Prepare the destination first

In `OWNER/SDK`, create and commit a normal Kaji recipe whose input is the chosen
relay target. For example, if `--target specs/acme.yaml` will be used:

```json
{
  "openapi": { "input": "./specs/acme.yaml", "name": "Acme API", "version": "1.0.0" },
  "output": { "path": "./generated" },
  "packages": [
    { "language": "typescript", "path": "web", "name": "@acme/api",
      "plugins": [{ "name": "sdk", "transport": "fetch" }] }
  ]
}
```

Copy the initial specification to that target so local generation can be checked.
Add package release metadata as in [SDK automation](sdk-automation.md#1-declare-the-package-and-its-delivery-metadata).
Generate the packages, run their native checks, then preview/write the ordinary
`sdk init` bootstrap **in the destination repository**. Commit its configuration,
actions and workflows before enabling the source relay. Configure credentials in
both repositories that run App-token jobs; registry publishing belongs to the
SDK repository's release workflow.

### Prepare the source relay

From the API repository root, preview the source workflow:

```sh
kaji sdk connect --repository acme/sdk-repo \
  --spec api/openapi.yaml --target specs/acme.yaml \
  --auth app --dry-run
```

`--spec` is a file in the API repository; `--target` is the specification path
inside the destination repository. Keep both repository-relative. Rerun without
`--dry-run` only after reviewing the generated source workflow and editable
spec-sync action sources. Commit those files through your normal process.
The destination target must match `openapi.input` in its recipe; the relay does
not update that recipe for you.

Local bootstrap writes `.github/workflows/kaji-spec-sync.yml` and
`.github/actions/kaji-spec-sync/{action.yml,sync.mjs}`; broker mode also includes
its token helper.
The relay workflow watches source pushes to `--base` (default
`main`) and allows manual dispatch.
The same `--base` selects the destination PR
base; its review branch defaults to `codex/kaji-spec-sync`.

For example, add
`--base develop` when both repositories use `develop`.
If their branches differ,
edit the generated workflow to set the source trigger and destination action
`base` independently.
The upstream
[spec-sync action](../packages/spec-sync/action.yml) and
[relay implementation](../packages/spec-sync/sync.mjs) are available for review.


**Relay limit: one specification file.** Bundle external relative `$ref` files
(such as `./schemas/contact.yaml`) before connecting; siblings are not copied.
Internal `#/components/...` references remain supported.

### Select credentials and action sources

| Option | Required operator setup |
| --- | --- |
| `--auth app` | Register/install your App for selected repositories; configure `SDK_APP_CLIENT_ID` and `SDK_APP_PRIVATE_KEY` where jobs run. |
| `--auth broker --broker-url https://broker.example` | Deploy your broker with an approved source workflow/ref and explicit destination installation mapping. |
| `--auth token` | Configure `SDK_GITHUB_TOKEN` yourself with the needed destination permissions. |
| `--actions local` | Review and commit vendored spec-sync/token helper source. This is the default. |
| `--actions remote --action-ref OWNER/kaji@REF` | Use a published/pinned action implementation containing spec-sync support. |

Contents: write and Pull requests: write permit routine destination PR updates.
Installing workflow files remains an owner-controlled bootstrap commit. No
hosted Kaji App, broker endpoint, repository, installation or secret is created
by `connect`. See [App setup](github-app.md) and
[broker policy](github-app-broker.md) for the two App authentication arrangements.

Local scaffolding vendors helper source from the generator build/current
checkout. Remote actions and a CI-installed launcher come from their selected
published revisions. Those can differ: reviewing a new local helper does not
make it available in an older published launcher/action. Verify both versions,
or distribute a reviewed source build/fork as described in
[source customization](source-customization.md).

## Fetch a remote specification periodically

A scheduled generation workflow is useful when `kaji.json` fetches a remote
OpenAPI source whose changes do not produce repository push events. Preview the
schedule explicitly:

```sh
kaji sdk init --root generated --config kaji.json --auth app \
  --schedule '17 3 * * *' --bump minor --dry-run
```

Use the same options with `sdk sync` to update the scaffold. `--bump` accepts
`major`, `minor` or `patch` and is passed to the generated `sdk pr` command. The five-field cron
above schedules 03:17 UTC daily in the generated workflow. The schedule is opt-in;
a recipe pointing to a URL does not itself create a periodic poller. Commit the
reviewed workflow to the repository's default branch before expecting it to run.

Scheduled jobs use the ordinary recipe → generation → PR flow. Configure private
spec credentials through environment-backed source credentials in that job;
see [remote source recipes](cli/recipes.md). These are separate from App PR
credentials and registry publishing identity.

| Source | Bump policy |
| --- | --- |
| URL/artifact-only | Explicit `--bump` required until prior source snapshots support comparison |
| Local versioned specification | Omit `--bump` to use API-diff sizing |

The example chooses `minor`. Review that policy; choose `major` or `patch` when
appropriate. Scheduling does not create source snapshots.

GitHub schedules run on the default branch, can be delayed under load, and may
be disabled in inactive public repositories. The time is a requested schedule,
not a delivery deadline. Current platform behavior is described in
[GitHub's scheduled workflow reference](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#schedule).

## Inspect local and remote automation state

Local inspection does not contact GitHub:

```sh
kaji sdk status --root generated --json
```

Read-only remote inspection selects a repository explicitly:

```sh
kaji sdk status --remote --repository acme/sdk-repo --json
```

Remote status uses your authenticated GitHub identity to inspect:

- Generation/check/release and optional spec-sync workflows.
- Recent runs and SDK/specification PRs.
- Visible secret **names**, never values.

It makes no remote changes. A secret name does not prove the credential works;
a workflow file does not prove a successful run. Treat denied queries as
permission errors rather than evidence that an item is absent.

For two repositories, inspect the API repository's relay runs and the SDK
repository's specification PRs, generation runs and release runs separately.
Remote status is an operator diagnostic, not a continuously hosted monitor.

## Check the handoff that failed

| Symptom | Check |
| --- | --- |
| Relay workflow missing | Source scaffold was reviewed and committed; selected action revision contains spec-sync. |
| No specification PR | Source run, selected App installation/target repository, input path and destination target path. |
| Destination generation fails after spec merge | Recipe input points at the target; contract is self-contained; required private source credentials exist. |
| Generated SDK PR never appears | Destination `sdk init` workflow was committed and its PR identity has destination access. |
| Scheduled fetch does not run | Explicit schedule was scaffolded/committed on default branch, workflow is enabled and remote source credentials are configured. |
| Remote status cannot list secrets | Inspect permission errors; secret-list access differs from workflow visibility. |
| Publication fails after successful relay | Follow [registry trust and exact-tag retry](sdk-publishing.md); relay credentials do not authorize publishing. |

Continue with [full SDK delivery](sdk-automation.md),
[registry publication](sdk-publishing.md), or
[author middleware and source customization](sdk-customization.md).
