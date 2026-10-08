# Generate, deliver and release an SDK

Follow one generated package from a local recipe to a published release.
Start with [generation](getting-started.md) if you need a working `kaji.json`.
Registry configuration and publication recovery are in [SDK publishing](sdk-publishing.md).

| Step | Result |
| --- | --- |
| Declare metadata and verify locally | Generated package with build/test commands |
| Preview and commit bootstrap | Reviewed generation, CI and release workflows |
| Configure authentication and registry trust | Separate identities for repository PRs and publication |
| Merge SDK and release PRs | Immutable package tag |
| Check and publish that tag | Verified registry release |

**On this page:** [Layout](#choose-the-repository-layout) · [Recipe](#1-declare-the-package-and-its-delivery-metadata) · [Bootstrap](#3-preview-the-workflow-bootstrap) · [Authentication](#5-configure-github-authentication) · [SDK PR](#6-open-and-review-the-sdk-content-pr) · [Release](#7-merge-release-and-publish-the-exact-tag) · [Troubleshooting](#diagnose-the-boundary-that-failed)

## Choose the repository layout

| Layout | API source, recipe and custom source | Generated code and releases | Bootstrap |
| --- | --- | --- | --- |
| One repository | API repository | Same repository, under `output.path` | Commit the generated code and local workflow/action files together. |
| Separate SDK repository | API repository | SDK repository, retaining `output.path` | Commit destination setup files there before enabling source generation. |
| Repository per language | API repository | One destination per language, retaining package paths | Commit each destination’s staged setup independently. |

| Routing option | Rule |
| --- | --- |
| `--repository OWNER/SDK` | One shared SDK destination |
| `--repository-pattern 'acme/api-{lang}'` | Exactly one `{lang}`; packages of the same language share a destination |

Choose one option. Per-package configuration routing is not implemented;
release metadata identifies packages within the selected repositories.

**Paths used below:** `output.path` is `generated`; the package path is `web`.
Use `--root generated --package web` for package commands, and the complete
`generated/web` path when selecting a release tag.

For a specification-only handoff, scheduled remote fetching or read-only GitHub
inspection, see [GitHub Actions for SDK authors](github-actions.md). Use a
launcher containing those commands/options; a current source checkout and an
older published launcher are not interchangeable.

## 1. Declare the package and its delivery metadata

A complete starting recipe looks like this:

```json
{
  "openapi": { "input": "./openapi.yaml", "name": "Acme API", "version": "1.0.0" },
  "output": { "path": "./generated" },
  "packages": [
    {
      "language": "typescript",
      "path": "web",
      "name": "@acme/api",
      "plugins": [{ "name": "sdk", "transport": "fetch", "client_name": "Acme" }],
      "release": {
        "build": [
          { "program": "npm", "args": ["install", "--ignore-scripts"] },
          { "program": "npm", "args": ["run", "build"] }
        ],
        "test": [{ "program": "npm", "args": ["run", "build"] }],
        "publisher": { "registry": "npm", "release_type": "node" }
      }
    }
  ]
}
```

Replace `@acme/api` and the specification path with your own values.

**The sample test phase checks compilation only.** Before publishing, add tests
for generated methods, middleware, errors and important wire formats. Declare
their command in `test`; ship maintained test sources through
[source overlays](sdk-customization.md) or a native test plugin.

`release` emits `<package>/.kaji/package.json`: the package's delivery contract.
Build/test commands run in the package directory, using executable and argument
vectors without shell interpolation. If you need pipelines or several shell
operations, declare a reviewed script as the command. Metadata can also be
supplied by native `release::metadata::<YourLanguage>(...)` plugins.

A package `version` explicitly pins its version. Without that override,
release-enabled regeneration retains the metadata version updated by Release
Please; cross-repository sync also retains the destination's released version.
The publisher verifies that metadata and the native manifest agree. Do not reset
an already released SDK to the API document's version accidentally.

## 2. Verify local generation before installing automation

From the API repository root:

```sh
kaji check openapi.yaml
kaji generate --config kaji.json
kaji generate --config kaji.json --check --format json
kaji sdk list --root generated --json
kaji sdk run --root generated --package web --phase build
kaji sdk run --root generated --package web --phase test
```

| Command | Checks or changes |
| --- | --- |
| `check` | API contract diagnostics |
| `generate` | Writes owned generated output |
| `generate --check` | Reports drift without writing; should be empty after generation |
| `sdk list` | Emitted delivery metadata |
| `sdk run --phase build/test` | Runs declared commands locally; requires the native toolchain and dependencies |

For changed or removed owned files, see [safe regeneration](safe-regeneration.md).

Inspect the exported API, native manifest, `.kaji/package.json` and generated
README. Confirm the package name, version, entry points and publisher. A
successful SDK build does not establish GitHub App access or registry trust.

## 3. Preview the workflow bootstrap

```sh
kaji sdk init --root generated --config kaji.json --auth app --dry-run
```

Review the reported local files before writing them. Then run the same command
without `--dry-run`, specifying the launcher version you intend CI to install:

```sh
kaji sdk init --root generated --config kaji.json \
  --auth app --kaji-version VERSION
kaji sdk status --root generated --json
```

`--base` defaults to `main`. With `init`/`sync`, it selects the source workflow's
push branch, generated SDK PR base and Release Please target branch. If your
source and SDK repositories use different branches, edit those independent
values in the reviewed scaffold. `--schedule '17 3 * * *' --bump minor` adds an
explicit periodic fetch and release-size policy; see the
[scheduled fetching guide](github-actions.md#fetch-a-remote-specification-periodically).

Replace `VERSION` with a published Kaji launcher containing these commands. A
local implementation does not publish that launcher for you. For a source build
or fork, change the readable generated workflow's launcher command to the binary
or package you actually distribute.

`init` and `sync` write local scaffold files. Neither command creates repositories,
Apps, installations, secrets, trusted publishers, pull requests or registry
versions. `sdk status` describes local package/workflow state; it does not verify
remote credentials or installation permissions.

| Generated file | Responsibility |
| --- | --- |
| `.github/workflows/kaji-sdks.yml` | Regenerate and open/update the SDK content PR. |
| `.github/workflows/kaji-sdk-ci.yml` | Check generated SDK packages. |
| `.github/workflows/kaji-sdk-release.yml` | Release Please orchestration, tag checks and publication. |
| `.github/actions/kaji-check/` | Readable language setup and package check helper. |
| `.github/actions/kaji-publish/` | Readable standard-registry publication helper. |
| `release-please-config.json` and `.release-please-manifest.json` | Per-package release strategies and current versions. |

Files depend on the publishers and repository layout.

| Action source | Setup |
| --- | --- |
| `--actions local` (default) | Review and commit vendored helpers; broker mode includes token-client sources |
| `--actions remote --action-ref OWNER/kaji@REVISION` | Select a reviewed, pinned remote implementation |

Scaffolding does not create a hosted service.

## 4. Commit the bootstrap in the correct repository

### Same repository

Commit the specification, recipe, custom author sources, generated output and
reviewed bootstrap files through your normal repository process. Set up App
credentials and registry trust before enabling the release workflow. Subsequent
source changes use an SDK PR rather than committing unrelated handwritten work.

### Separate SDK repository

Preview and write the source-side bootstrap with the destination selected:

```sh
kaji sdk init --root generated --config kaji.json \
  --repository acme/sdk-repo --auth app --dry-run
kaji sdk init --root generated --config kaji.json \
  --repository acme/sdk-repo --auth app --kaji-version VERSION
```

The source generation workflow stays in the API repository. Destination
checks/release workflows, actions and release configuration are staged under
`.kaji/sdk-repository-setup/`. Copy **the contents of that directory**, including
hidden files, into the SDK repository root. For sibling checkouts named
`api-repo` and `sdk-repo`, run this **from the SDK checkout**:

```sh
cp -R ../api-repo/.kaji/sdk-repository-setup/. .
git diff --stat
```

Review the resulting files and commit them there first. Commit the API repository's source generation workflow, recipe
and author sources separately. Keep a committed destination default branch for
the first content PR to target.

The destination still receives `generated/web`, not `web`: setup paths follow
the recipe's `output.path`. Do not flatten the copied package directory or install
the destination release workflow in the API repository. Routine SDK App tokens
have Contents/PR access and cannot bootstrap or replace workflow definitions;
the initial setup is an owner-controlled commit.

After package additions/removals or workflow changes, use `sdk sync` with the
same options to refresh local setup. The package matrix is captured during
scaffolding. Edited scaffold files require a manual merge; setup does not silently
overwrite your edits or reset released versions in the release manifest.

### Repository per language

Preview the source workflow and all destination setup files together:

```sh
kaji sdk init --root generated --config kaji.json \
  --repository-pattern 'acme/api-{lang}' --auth app --dry-run
kaji sdk init --root generated --config kaji.json \
  --repository-pattern 'acme/api-{lang}' --auth app --kaji-version VERSION
```

The example routes TypeScript to `acme/api-typescript` and Python to
`acme/api-python`. Each source job obtains destination-scoped access and runs
`sdk pr --language LANG` for every package of that language.

Output paths stay intact: `generated/web` remains `generated/web` in its
TypeScript repository.

Setup is staged separately at
`.kaji/sdk-repository-setup/OWNER/REPO/`. Copy each repository directory’s contents,
including `.github`, into that destination’s root, then review and commit there.
For example, from the `api-typescript` checkout:

```sh
cp -R ../api-repo/.kaji/sdk-repository-setup/acme/api-typescript/. .
git diff --stat
```

Each directory contains its check/publish helpers, CI and release workflows, and
Release Please configuration/manifest.
Create the repositories and their base
branches yourself.
Give the App installation access to every selected destination,
and configure registry publishers and the `release` environment independently in
each one.

Routine Contents/Pull requests tokens cannot install workflow files;
bootstrap requires the repository owner’s commit or separately authorized workflow
permissions.
The scaffold does not create repositories or install an App remotely.

Instead of copying into a checkout, explicitly ask Kaji to prepare a destination
bootstrap PR:

```sh
kaji sdk install --setup .kaji/sdk-repository-setup/acme/api-typescript \
  --repository acme/api-typescript --base main \
  --branch codex/kaji-sdk-setup --dry-run
kaji sdk install --setup .kaji/sdk-repository-setup/acme/api-typescript \
  --repository acme/api-typescript --base main \
  --branch codex/kaji-sdk-setup
```

Review the dry-run plan before the second command.
Installation uses the
invoker’s GitHub authentication and needs authority to commit workflow files;
the routine SDK App’s Contents/Pull requests token is insufficient for that
bootstrap.
It opens a reviewable PR, without a direct commit to the base branch or
force push.

Merge it through the destination’s review process.
Repeat for every
destination.
The API repository’s source generation workflow still needs its
owner’s commit separately.

The installation inventory protects edited helper/workflow files. Review
refresh conflicts and merge intended changes. The example has not been installed
remotely.

Refresh with `sdk sync` and the same pattern, authentication, base, schedule and
bump options. When source and destination base branches differ, edit the generated
workflow values separately. A broker’s allowlist must explicitly cover each
source-to-destination mapping. See [publishing trust](sdk-publishing.md#configure-trust-before-the-first-publication).

## 5. Configure GitHub authentication

| Mode | Workflow identity | Operator setup |
| --- | --- | --- |
| `--auth app` | Short-lived installation token minted from your App | Register/install your private App; set `SDK_APP_CLIENT_ID` variable and `SDK_APP_PRIVATE_KEY` secret. |
| `--auth broker --broker-url HTTPS_URL` | GitHub OIDC exchanged by your broker for a scoped installation token | Register/install an App, deploy the broker, and approve exact source workflows and destination mappings. |
| `--auth token` | `SDK_GITHUB_TOKEN` secret | Provision a token with the required repository permissions yourself. |

Preview an App registration manifest with
`kaji sdk app --name "Acme SDK Sync" --dry-run`, then follow
[GitHub App setup](github-app.md) to register and install it. The App needs Contents: write and
Pull requests: write for the selected repositories; routine workflows do not need
workflow-write permission. App-token acquisition/revocation belongs to the job.
Registry publication uses its own identity and configuration.

For separate repositories, configure the App variables/secrets in the repositories
running generation and release jobs (or grant both repositories access to the
organization-level values). Destination release jobs need their own working
App-token configuration, even when source generation already succeeds.

Broker mode keeps the App key at your deployed broker rather than in Actions
secrets. [Broker deployment and policy](github-app-broker.md) explain its signed
OIDC verification, exact repository/workflow/ref allowlists and installation
mapping. Approve both the source generation workflow and the destination
release workflow identities when both exchange tokens. Kaji provides its source; a public hosted App or endpoint is not created
by scaffolding. Terraform hosting remains a plan, not provisioned infrastructure.

## 6. Open and review the SDK content PR

```sh
kaji sdk diff --base previous-openapi.yaml --head openapi.yaml --json
kaji sdk pr --config kaji.json --repository acme/sdk-repo --dry-run
kaji sdk pr --config kaji.json --repository acme/sdk-repo --bump minor
```

The PR dry run prints the planned target and release-size selection; it does not
execute generation or validate target authentication. The real command requires
a clean source checkout, Git and authenticated `gh`. It generates in temporary
clones and synchronizes inventoried output into one destination branch. Unowned
files survive; modified owned files fail rather than being overwritten. Existing
PR ancestry/manual commits survive. Normal pushes reject races instead of forcing
branch history.

| Setting | Default or behavior |
| --- | --- |
| PR branch / base | `codex/kaji-sdks` / `main`; override with `--branch` / `--base` |
| Suggested bump | Breaking API change → major; other API change → minor; generation-only change → patch |
| Comparison base | `--base-spec-ref`, GitHub push before revision, or `HEAD~1` |
| URL/artifact-only source | Requires explicit `--bump` until prior snapshots are supported |

Review the bump: a schema diff cannot capture every behavioral change.

## 7. Merge, release and publish the exact tag

```text
Source change
  -> generated SDK content PR and checks
  -> owner merges SDK PR
  -> Release Please version/changelog PR
  -> owner merges release PR
  -> existing immutable package tag
  -> checks on that tag and retained package artifacts
  -> protected release environment
  -> registry publication and verification
```

Release Please updates native versions and release metadata. Its strategy must
support your manifest; inspect `release_type` and `extra_files` when adding a
language. Standard publisher names are `npm`, `pypi`, `crates.io` and `go`; other
registries and explicit publisher commands use the custom path described in
[SDK publishing](sdk-publishing.md).

The release matrix reads metadata from each tag, not today's main branch.
Checks
run without publication credentials.
The publication job checks out the same tag
and restores its tested output.
All release checks gate publishing.
Native fallback
checks can be compile/lint-only; declare real behavioral tests in metadata.

Swift uses a macOS check runner.
Community toolchains need explicit setup steps.
Configure the registry's trusted publisher and GitHub `release` environment before
allowing a publication job to proceed.

## Retry one failed publication

After correcting authentication or transient infrastructure failure, dispatch
`kaji-sdk-release.yml` with both `path` and the **existing exact `tag`**. Inspect the
selection locally first, from the SDK repository:

```sh
kaji sdk releases --path generated/web --tag EXISTING_PACKAGE_TAG
```

Use the tag actually created by your release configuration; component names are
sanitized and Go uses directory prefixes. The command resolves the release
matrix without publishing. Dispatching the workflow with that path/tag selects
one package and bypasses creation of another release PR. Preserve the original
tag and artifacts; changed bytes under the same version are rejected by standard
publishers. Custom commands own their own retry/idempotency policy.

## Diagnose the boundary that failed

| Symptom | First check |
| --- | --- |
| Package missing from `sdk list` | Generation emitted its `.kaji/package.json`; `--root` is the actual output root. |
| Drift or owned-file conflict | Run `generate --check --format json`; change recipe/author source rather than overwriting materialized owned files. |
| Stale CI matrix | Refresh `sdk sync`, review edited scaffolds, commit destination setup again if needed. |
| SDK PR denied | App installation includes the destination; token has Contents/PR write; source checkout is clean and `gh`/Git use that identity. |
| Release PR/tag missing | Release Please configuration, component/version manifest and authenticated workflow identity. |
| Publish identity rejected | Registry trust matches the destination repository, actual release workflow filename and `release` environment. |
| Existing-version digest mismatch | Original retained artifact vs rebuilt bytes; fix reproducibility or issue a new version instead of moving the tag. |
| Custom registry fails | Its declared command, native toolchain, environment credentials and custom verification logic. |

Continue with [publishing setup and registry rules](sdk-publishing.md),
[SDK author customization](sdk-customization.md), or
[plugin authoring](typed-plugins.md).

## Diagnose an existing delivery setup

```sh
kaji sdk doctor --root generated --json
kaji sdk inspect --root generated --json
```

`doctor` checks the generation ownership inventory, package delivery metadata,
expected toolchain commands and workflow files without changing or building the
output. Add `--repository OWNER/REPO` for optional remote configuration-name
checks through `gh`. It reports remediation steps without returning credential
values. `inspect` describes emitted artifacts and ownership; it is not a
pre-generation plugin dependency graph inspector.

API diffs now include deterministic structured change entries and Markdown
notes. When `sdk pr` computes a local source diff, bounded, sanitized API notes
are embedded into its commit and matching squash-merge override for Release
Please. The selected release bump controls the outer commit title. Supplying
an explicit `--bump` to `sdk pr` bypasses source comparison, so no inferred API
notes are available for that invocation.

For a disposable end-to-end verification, use the prepared
[delivery workflow template](../packages/sdk-delivery-test/README.md). It defaults
to preview and requires configured allowlists and a protected environment before
opening a test SDK PR. Registry publication follows the destination's regular
reviewed release workflow. This template has not been dispatched or published
as part of these changes.

## Local source and publication-gate verification

The repository regression `all_sdk_languages_receive_readable_checks_and_gated_publish_sources`
constructs separate SDK repository scaffolds for TypeScript, Python, Go, Rust,
Java, C#, Swift, PHP, Ruby and Elixir. Every destination gets editable local check
and publish action sources, CI metadata for its language, and a release workflow.
Swift checks select macOS; the other built-in profiles select Ubuntu. The test
checks that native checks run against the selected immutable release tag, precede
publication, and receive no publication OIDC permission. Only the publication job
requests OIDC and targets the `release` environment. Configure reviewers and
registry trust for that environment in GitHub; emitting YAML does not create them.

Standard publication applies to npm, PyPI, crates.io and Go source tags. The other
profiles require reviewed custom publisher commands. A generated action source is
not proof that a Maven/NuGet/RubyGems/Packagist/Hex endpoint is configured. The local
matrix verifies scaffold structure and isolation; SDK compilation tests verify
language generation separately. The disposable delivery workflow remains opt-in
and does not establish live registry/GitHub acceptance until an owner supplies
allowlisted disposable destinations and explicitly runs it.
