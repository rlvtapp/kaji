# GitHub Actions for SDK authors

Use GitHub Actions to deliver changes while keeping the specification, generator
recipe, middleware sources and automation readable in your repositories. This
page explains four repository arrangements. [SDK automation](sdk-automation.md)
contains the full content-PR → release-PR → exact-tag publication journey.

> Use a Kaji launcher containing the commands/options shown here. The current
> source checkout and a previously published launcher/action can differ; verify
> the version you install in CI before committing its scaffold.

## Choose where generation runs

| Your situation | Workflow arrangement | Where the recipe lives |
| --- | --- | --- |
| API and SDK code share a repository | `sdk init` generates local SDK/check/release workflows. | Same repository as the specification. |
| The API repository owns generation; SDK output lives elsewhere | `sdk init --repository OWNER/SDK` generates the source workflow and stages destination setup. | API repository; generated code is sent to the SDK repository. |
| The API repository owns generation; each language has its own repository | `sdk init --repository-pattern 'OWNER/api-{lang}'` writes source jobs per language and stages setup per destination. | API repository; each job delivers only its selected language. |
| The API repository only exports a specification; SDK authors own generation | `sdk connect` relays the spec as a destination PR; destination `sdk init` handles generation and release. | SDK repository, pointing at the relayed specification path. |

Do not enable two generation owners for the same output branch. Decide which
repository owns the recipe, package options and author source before bootstrap.
A relay moves the specification; it does not move generated SDKs, rewrite the
recipe or deploy a hosted generation service.

## Bootstrap generation and release in one repository

First generate and verify your packages locally:

```sh
kaji generate --config kaji.json
kaji generate --config kaji.json --check --format json
kaji sdk list --root generated --json
kaji sdk init --root generated --config kaji.json --auth app --dry-run
```

Inspect the planned files, then rerun without `--dry-run` and with a Kaji launcher
version that actually contains these commands. Review and commit the generated
workflows/actions along with the recipe and author sources. `--base` defaults to
`main`; for `sdk init`/`sync` it selects the source push trigger, SDK PR base and
Release Please target branch. Use `--base your-branch` for another convention. Configure the App,
registry trust and protected `release` environment separately. The
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

The source workflow generates a language matrix. Each job uses a destination-scoped
token and `sdk pr --language LANG`; packages sharing a language share its
repository. Their original output root and package directories are preserved.
Commit the source recipe, bundled author sources and workflow in the API repository.

Each destination’s bootstrap is local staging under
`.kaji/sdk-repository-setup/OWNER/REPO/`: check/publish helpers, CI/release workflows
and Release Please configuration/manifest. Copy that directory’s **contents** into
the matching destination and commit them with owner/workflow authority before the
first SDK PR. The routine App needs Contents/Pull requests access; it does not
bootstrap workflows. No repository, hosted service or live App installation is
created by this command. Configure registry trust separately for each repository.
An explicit `sdk install --setup .kaji/sdk-repository-setup/acme/api-typescript
--repository acme/api-typescript --dry-run` previews a bootstrap PR alternative to
manual copying. Rerun without `--dry-run` only when you intend to open that PR with
GitHub credentials authorized to write workflow files. This does not install the
API repository’s source generation workflow; its owner commits that separately.
Tracked installed files preserve edits and surface conflicts on refresh. See the
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
its token helper. The relay workflow watches source pushes to `--base` (default
`main`) and allows manual dispatch. The same `--base` selects the destination PR
base; its review branch defaults to `codex/kaji-spec-sync`. For example, add
`--base develop` when both repositories use `develop`. If their branches differ,
edit the generated workflow to set the source trigger and destination action
`base` independently. The upstream
[spec-sync action](../packages/spec-sync/action.yml) and
[relay implementation](../packages/spec-sync/sync.mjs) are available for review.


The relay carries one specification file. External relative `$ref` documents,
for example `./schemas/contact.yaml`, are unsupported because their sibling
files are not copied to the destination. Bundle those references into a
self-contained contract before connecting. Internal `#/components/...`
references stay in the same document. The relay is not a generic repository
mirror or dependency-copy mechanism.

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

A scheduled job reuses the normal recipe/generation/PR path. Configure private
spec credentials as environment-backed source credentials in that job, following
[remote source recipes](cli/recipes.md). Fetching credentials are distinct from
App credentials for a destination PR and from registry publishing identity.
The example explicitly chooses a `minor` bump policy for remote updates. Review
that choice for your contract: use `--bump major` for a policy requiring major
releases, or `--bump patch` for patch-only updates. URL/artifact-only sources need
an explicit bump until prior source snapshots support automatic comparison; a
schedule does not create those snapshots. For local versioned specifications,
omit `--bump` when you want the existing API-diff sizing path.

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

It inspects generation/check/release workflows, the optional spec-sync workflow,
recent runs, SDK/specification pull requests and visible secret names using
your authenticated GitHub identity. It does not return secret values, create
secrets, install an App, repair workflows, approve PRs or dispatch publishing.
A listed secret name is not proof its value works; a workflow file is not proof
it has run successfully. Visibility depends on that identity's permissions, so
a denied query is not evidence that the remote item is absent.

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
