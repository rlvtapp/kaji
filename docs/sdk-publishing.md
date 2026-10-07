# Publish and recover an SDK release

This guide is for SDK authors configuring the final registry step. Begin with
[SDK automation](sdk-automation.md) to generate packages, bootstrap repositories,
configure GitHub authentication and create release tags. Publishing does not
regenerate the SDK, create its tag, register an account or establish registry trust.

## Kaji's own repository releases

Kaji's root release workflow prepares Release Please pull requests on `main`.
It uses the simple strategy and explicit TOML updates because member crates inherit
`workspace.package.version`. `version.txt` tracks the source version; the release
manifest tracks the last published version, currently `0.4.0`. Minor features bump
the minor version before 1.0, so the pending feature release is `0.5.0`.

`KAJI_RELEASE_ENABLED` must be set to `true` in repository variables before this
workflow can create a GitHub release/tag. Leave it unset while release preparation
is paused. Tag-triggered publication and manual publication remain deliberate
release actions. The version-update regression test runs the pinned Release Please
updaters against manifests and lockfiles without calling GitHub or publishing.

The lockfile selector addresses Release Please's tagged TOML name values. When
adding a workspace crate, update that selector; the regression test requires every
member to change and every external dependency to remain unchanged.

## Decide which publisher owns the release

The package's `.kaji/package.json` declares a publisher. Use `npm`, `pypi`,
`crates.io` or `go` with empty/omitted `commands` for a standard publisher.
**Explicit nonempty commands select the custom path**, even if the registry name
is one of those four. Other registries require commands.

| Package ecosystem | Standard path | Native prerequisite | Author setup |
| --- | --- | --- | --- |
| TypeScript / JavaScript | npm | `package.json`, built entry points, publishable package | Own the npm package/scope and configure GitHub trusted publishing. |
| Python | PyPI | Static project name/version in `pyproject.toml`; wheel/sdist in `dist` | Own the project or create a pending publisher; configure GitHub trust. |
| Rust | crates.io | Publishable single-package `Cargo.toml` and packaged sources | Own the crate; configure its GitHub trusted publisher. |
| Go | Public module proxy | Correct `go.mod`, reachable repository and canonical pushed source tag | Choose the public module identity and repository layout before releasing. |
| Java | Custom Maven/registry commands | Native build, publication configuration and signing when required | Supply registry credentials, artifact coordinates and release strategy. |
| C# / .NET | Custom NuGet commands | Built `.nupkg`, package identity/version and selected feed | Supply feed identity, authentication and command verification. |
| PHP / Ruby / Elixir / Swift / community targets | Custom commands or source-tag distribution | Ecosystem-specific publishable output | Declare commands and toolchains; choose a compatible Release Please strategy. |
| Private/custom registry in any ecosystem | Custom commands | Registry-specific package and endpoint | Configure credentials and idempotent upload/verification yourself. |

Choosing `publisher.registry: "npm"` does not redirect the standard helper to a
private npm server. Standard helpers use fixed public registry endpoints and npm
public access. Use explicit custom commands for private packages/feeds, alternate
endpoints, release staging or other publication policies.

## Keep metadata and native manifests aligned

A generated Python delivery contract might be:

```json
{
  "schema_version": 1,
  "language": "python",
  "name": "python-sdk",
  "version": "1.0.0",
  "build": [{ "program": "python", "args": ["-m", "build"] }],
  "test": [{ "program": "python", "args": ["-m", "unittest", "discover"] }],
  "publisher": {
    "registry": "pypi",
    "release_type": "python",
    "commands": [],
    "extra_files": []
  }
}
```

Author this through the recipe's `release` object or a native release metadata
plugin, rather than editing the emitted bookkeeping file. `name` identifies the
release component. The publisher obtains the registry package name and version
from the native manifest and verifies its version against this metadata.
`release_type` and `extra_files` configure Release Please manifest updates, not
registry authentication. An arbitrary strategy name is not automatically a new
Release Please implementation; review the generated release configuration.

Builds and tests should exercise the package's actual public methods and wire
behavior. A compile/lint fallback is useful validation but cannot prove response
handling, bundled middleware or compatibility with an API. See
[author customization](sdk-customization.md) to ship maintained test sources.

## Configure trust before the first publication

Use the **destination SDK repository**, `.github/workflows/kaji-sdk-release.yml`
and the `release` environment from the generated workflow when configuring trust.
If you rename these files or change the environment, update registry trust too.
GitHub App credentials used to create SDK/release PRs do not grant registry access.

With `sdk init`/`sync --repository-pattern 'acme/api-{lang}'`, configure trust
against each resulting destination independently: for example npm trusts
`acme/api-typescript` while PyPI trusts `acme/api-python`. Use the exact workflow
filename `kaji-sdk-release.yml` and environment `release` from that destination’s
staged workflow. Copy and commit its bootstrap first; local staging does not
register a registry publisher or install repository permissions.

The pattern groups packages by language and retains their full output paths.
Several packages in one destination still need their own registry identities and
publisher settings. Go releases publish canonical repository/module tags; they do
not use a registry upload token. Custom registries for the other languages require
publisher credentials and edited action configuration as described below; an App
installation token is not a registry credential.

### npm

1. Own the package/scope and open the package's Trusted Publisher settings on npm.
2. Choose GitHub Actions; enter the owner, repository and workflow **filename**
   `kaji-sdk-release.yml`, plus `release` when restricting trust to that environment.
3. Allow `npm publish` for this helper. A trust configuration permitting only
   staged publication does not authorize its direct publish operation.
4. Use GitHub-hosted runners and a compatible npm CLI (at least 11.5.1) / Node
   (at least 22.14.0). The Kaji action selects Node 24; confirm its npm version
   when changing the toolchain.

The registry's current setup and allowed-action controls are documented in
[npm trusted publishing](https://docs.npmjs.com/trusted-publishers/).
Package/account creation and any initial release needed to establish ownership
remain operator tasks; scaffolding does not perform them.

### PyPI

Open the project's Publishing settings and add a GitHub publisher matching owner,
repository, workflow filename and environment. For a new project, use PyPI's
pending-publisher flow. Configure a protected GitHub environment with the same
name. The Kaji helper uses the official PyPA upload action with OIDC on Linux;
it does not select TestPyPI or an alternate index. Follow
[PyPI publisher setup](https://docs.pypi.org/trusted-publishers/adding-a-publisher/)
and [using a publisher](https://docs.pypi.org/trusted-publishers/using-a-publisher/).

### crates.io

Configure the crate's Trusted Publishing settings for the repository, workflow
filename and selected environment. The helper obtains a temporary token through
[the official crates.io authentication action](https://github.com/rust-lang/crates-io-auth-action)
and publishes using Cargo. For a new crate, establish ownership and check the current initial-publish
requirements in [crates.io trusted publishing](https://crates.io/docs/trusted-publishing)
before enabling automation; this helper does not register trust for a new name. Check ownership and `publish` restrictions in
`Cargo.toml`; a workspace needing coordinated multi-crate publishing requires a
custom command path. See [Cargo publication](https://doc.rust-lang.org/cargo/commands/cargo-publish.html).

### Go

Go publication makes a source version available through its repository and public
proxy; there is no SDK archive-upload credential exchange. Verify the module's
canonical repository path and release tag prefix, push the immutable tag, then
let the helper request and verify the module version. Private modules require
private-module/proxy configuration outside the standard public helper. The
[Go module reference](https://go.dev/ref/mod#vcs-version) defines the source-tag rules.

## Publish exactly what passed checks

The generated release workflow checks each release tag without publishing
credentials, retains package artifacts, then checks out that same tag in the
publication job and restores those artifacts. Every release check gates the
publishing job. Configure the GitHub `release` environment's protections before
enabling it. The standard action requires an existing tag whose commit equals
checkout HEAD; it never creates or pushes a tag.

For custom workflow integration, the essential job shape is:

```yaml
# Include checkout of the exact existing tag, artifact restoration and native
# toolchain setup before this step.
permissions:
  contents: read
  id-token: write
environment: release
steps:
  - uses: ./.github/actions/kaji-publish
    with:
      registry: pypi
      path: generated/python
      tag: python-sdk-v1.0.0
```

This is a job fragment, not a complete workflow. The vendored action installs
Node 24 and, for Python, Python 3.12. The caller provides Cargo/Go toolchains.
Do not run arbitrary package build steps after granting publication credentials;
use the outputs retained from the checked tag.

## What a standard publisher verifies

| Registry | Upload/index operation | Existing-release verification |
| --- | --- | --- |
| npm | Pack without lifecycle scripts; validate packed entry points; publish the exact tarball with public access/provenance | Selected tarball SHA-512 integrity must match. `npm-tag` defaults to `latest`. |
| PyPI | Inspect wheel/sdist metadata without executing it; upload selected files with the PyPA action | Every selected filename's SHA-256 must match. Missing files in a partial upload can resume. `dist-dir` defaults to `dist`, relative to the package. |
| crates.io | Cargo package/verify and publication using temporary registry identity | Crate SHA-256 must match. Cargo republishes by packaging the manifest checkout, which must remain unchanged. |
| Go | Request canonical version from the public proxy | Module path and version must agree. Source-tag identity replaces local archive digest comparison. |

An existing registry version succeeds only when the corresponding verification
passes. Authentication failures and outages are not classified as duplicate
versions. Differing bytes under the same version fail; rebuilds must reproduce
the original artifact if a retry requires rebuilding. npm uploads its staged
tarball; Python uploads retained distributions; Cargo packages its checkout.
No npm lifecycle hooks run during pack/publish: build hooks belong in the earlier
build phase.

## Go tag prefixes: decide these before bootstrap

A tag prefix follows the module directory relative to the repository, not just
its import path. For semantic major versions two and above, `/vN` belongs in the
module path; a trailing source directory `vN` is excluded from the directory
prefix. [Go source version rules](https://go.dev/ref/mod#vcs-version) specify this.

| Source directory | Module path | Version | Required tag |
| --- | --- | --- | --- |
| `.` | `github.com/acme/sdk` | `1.0.0` | `v1.0.0` |
| `generated/go` | `github.com/acme/sdk` | `1.0.0` | `generated/go/v1.0.0` |
| `generated/go/v2` | `github.com/acme/sdk/v2` | `2.0.0` | `generated/go/v2.0.0` |
| `v2` | `github.com/acme/sdk/v2` | `2.0.0` | `v2.0.0` |

Inspect the generated Release Please configuration for these prefixes. A normal
hyphenated component tag is insufficient for a Go submodule.

## Custom commands and editable sources

For Maven, NuGet or another registry, configure executable/argument vectors:

```json
{
  "publisher": {
    "registry": "your-registry",
    "release_type": "simple",
    "commands": [{ "program": "node", "args": ["scripts/publish.mjs"] }],
    "extra_files": []
  }
}
```

Provide that script through an author source overlay. This fragment supplies a
custom publication entry point; it does not implement upload, authentication,
manifest updates or artifact verification for your registry. Run it locally with
`kaji sdk run --root generated --package web --phase publish` **only when you
intend a live publication**. Build/test phases are separate commands.

Custom publishers own credential setup, immutable version checks, duplicate-race
handling, partial upload recovery and final registry verification. Their toolchain
or environment dependencies need explicit workflow setup. Credentials belong in
the protected publication environment; do not hardcode them in metadata, source,
arguments or logs. Map protected-environment secrets explicitly into the custom publish step's
`env`, then read them inside your script. Environment membership does not export
all secrets to subprocesses automatically, and command argument vectors do not
expand `$TOKEN` placeholders. Keep the generated workflow edit reviewable.
`extra_files` must cover the manifests your selected release strategy needs to
update.

By default, `sdk init`/`sync` vendor readable helpers under
`.github/actions/kaji-publish/`. Review and edit `action.yml` and `publish.mjs` as
normal repository source. Subsequent scaffold refreshes require manual merges
for edited files. Remote-action mode can point at your pinned fork instead.
Standard helper command failures omit captured subprocess output to avoid
exposing credentials; reproduce checks locally without publication credentials
when more diagnostics are needed.

## Retry without changing release identity

Dispatch `kaji-sdk-release.yml` with the exact existing `path` and `tag`. Preview
its one-package matrix from the destination checkout:

```sh
kaji sdk releases --path generated/python --tag python-sdk-v1.0.0
```

Use the tag your release configuration actually created. Correct registry trust
or transient failures, then retry the original tag with the original artifacts.
Do not force-move a release tag or upload different content as the same version.
For a changed package, make a new content/release PR and version. Custom publisher
commands must implement their own safe retry rules.

## Verification available in this repository

`node --test packages/sdk-publish/test/*.test.mjs` exercises argument handling,
digest comparisons, duplicate races, path boundaries, canonical Go tags and failed
registry requests using mocked registries/publishers. Set `KAJI_TEST_PYTHON` to a
Python 3.11+ interpreter for the archive fixture when the default Python is older.
These tests do not configure live registry trust or prove a live upload.

Continue with [delivery/bootstrap](sdk-automation.md),
[GitHub App credentials](github-app.md), or
[SDK author source customization](sdk-customization.md).
