# Generate, deliver and release an SDK

This guide is for the SDK author: the team that owns the OpenAPI document,
generator recipe, generated packages and releases. It follows one package from
local generation to a published version. Start with [generation](getting-started.md)
if you do not yet have a working `kaji.json`. For registry setup and retry details,
continue with [SDK publishing](sdk-publishing.md).

## Choose the repository layout

| Layout | API source, recipe and custom source | Generated code and releases | Bootstrap |
| --- | --- | --- | --- |
| One repository | API repository | Same repository, under `output.path` | Commit the generated code and local workflow/action files together. |
| Separate SDK repository | API repository | SDK repository, retaining `output.path` | Commit destination setup files there before enabling source generation. |
| Repository per language | API repository | One destination per language, retaining package paths | Commit each destination’s staged setup independently. |

Use `--repository OWNER/SDK` for one shared destination, or
`--repository-pattern 'acme/api-{lang}'` for one destination per language.
The pattern must contain exactly one `{lang}` placeholder; the two flags cannot
be combined. Several packages of the same language share its repository.
Per-package configuration routing is planned and is not implemented; the pattern
flag is the available routing interface. Release metadata identifies each package.

In this guide, `kaji.json` uses `output.path: "generated"`, and the TypeScript
package has `path: "web"`. Its repository-relative path is therefore
`generated/web`. Commands that select a package below `--root generated` use
`--package web`; release-tag selection uses the complete `generated/web` path.

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

Replace `@acme/api` with a package you own, and point the recipe at your actual
specification. The example's test phase checks compilation only. Before enabling
publication, add behavioral tests for the generated methods, customer middleware,
error handling and important wire formats, and declare their command in `test`.
Keep those test sources alongside the recipe and ship them through
[source overlays](sdk-customization.md), or use a maintained native test plugin.

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

`check` diagnoses the API contract. Generation writes owned output.
`generate --check` previews drift without writing; immediately after generation,
it should report no generated changes. Build/test commands execute locally, so
your language toolchain and declared dependencies must already be available.
See [safe regeneration](safe-regeneration.md) for changed or removed owned files.

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

Actual files depend on whether packages have publishers and whether the SDK
repository is separate. `--actions local` is the default: helpers are vendored,
committed source. `--actions remote --action-ref OWNER/kaji@REVISION` selects a
remote action implementation instead. In broker mode, local token-client sources
are also included. Pin the code you review; no Kaji hosted service is implied.

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

For TypeScript and Python this selects `acme/api-typescript` and
`acme/api-python`. The source workflow has a job per language; each job obtains
access scoped to its destination and runs `sdk pr --language LANG`. Every package
of that language is included. Recipe output paths remain intact: `generated/web`
stays `generated/web` in the TypeScript repository, even when it is the only
package there.

Setup is staged separately at
`.kaji/sdk-repository-setup/OWNER/REPO/`. Copy each repository directory’s contents,
including `.github`, into that destination’s root, then review and commit there.
For example, from the `api-typescript` checkout:

```sh
cp -R ../api-repo/.kaji/sdk-repository-setup/acme/api-typescript/. .
git diff --stat
```

Each directory contains its check/publish helpers, CI and release workflows, and
Release Please configuration/manifest. Create the repositories and their base
branches yourself. Give the App installation access to every selected destination,
and configure registry publishers and the `release` environment independently in
each one. Routine Contents/Pull requests tokens cannot install workflow files;
bootstrap requires the repository owner’s commit or separately authorized workflow
permissions. The scaffold does not create repositories or install an App remotely.

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

Review the dry-run plan before the second command. Installation uses the
invoker’s GitHub authentication and needs authority to commit workflow files;
the routine SDK App’s Contents/Pull requests token is insufficient for that
bootstrap. It opens a reviewable PR, without a direct commit to the base branch or
force push. Merge it through the destination’s review process. Repeat for every
destination. The API repository’s source generation workflow still needs its
owner’s commit separately.

Installed helper/workflow sources remain editable. The tracked installation
inventory protects destination edits when setup changes: review conflicts and
merge intentional changes instead of expecting regeneration to overwrite them.
No remote installation was performed while preparing this example.

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

The default branch is `codex/kaji-sdks`, targeting `main`; use `--branch` and
`--base` for your repository's conventions. API diffs suggest major for breaking
changes, minor for other API changes, patch for generation-only changes. Automatic
comparison uses `--base-spec-ref`, a GitHub push's before revision, or `HEAD~1`.
Remote URL/artifact-only specifications require an explicit `--bump` until prior
source snapshots are supported. Review suggested release sizes rather than
assuming a schema diff captures every behavioral change.

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

The release matrix reads metadata from each tag, not today's main branch. Checks
run without publication credentials. The publication job checks out the same tag
and restores its tested output. All release checks gate publishing. Native fallback
checks can be compile/lint-only; declare real behavioral tests in metadata.
Swift uses a macOS check runner. Community toolchains need explicit setup steps.
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
