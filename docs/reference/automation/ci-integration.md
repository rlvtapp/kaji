# CI integration

Poolster ships repository-owned integrations for GitHub Actions and GitLab CI. Both
install the published npm launcher, so CI needs Node but not Rust or Go.

**On this page:** [GitHub](#github-actions) · [GitLab](#gitlab-ci) · [Marketplace](#github-marketplace) · [Poolster release](#publishing-poolsters-npm-packages) · [SDK repos](#generated-sdk-repositories)

## GitHub Actions

```yaml
steps:
  - uses: actions/checkout@v4
  - uses: rlvtapp/poolster/packages/integrations/github@main
    with:
      config: api/poolster.json
```

The action accepts `config`, `version`, and `working-directory`. Pin the action
and npm `version` to a release in production. See the
[action README](../../../packages/integrations/github/README.md) for all inputs.

## GitLab CI

```yaml
include:
  - remote: 'https://raw.githubusercontent.com/rlvtapp/poolster/<poolster-release-tag>/packages/integrations/gitlab/poolster.yml'

generate-sdk:
  extends: .poolster:generate
  variables:
    POOLSTER_CONFIG: api/poolster.json
```

| Variable | Controls |
| --- | --- |
| `POOLSTER_CONFIG` | Recipe path |
| `POOLSTER_VERSION` | Launcher version |
| `POOLSTER_WORKING_DIRECTORY` | Working directory |

**Linux requires glibc.** The template uses Debian-based Node; Alpine/musl is
unsupported by the published launcher. See the
[template README](../../../packages/integrations/gitlab/README.md).

## GitHub Marketplace

The repository-local action can be used directly, but it cannot be listed in
GitHub Marketplace: Marketplace actions require one `action.yml` at the root of
a public action repository.

When Poolster is ready to publish a Marketplace action,
create a small dedicated public repository (for example `rlvtapp/poolster-action`)
whose root contains this action, release it under a stable tag, and publish that
release from GitHub.
Do not add a root action metadata file to this monorepo
solely for Marketplace discovery.

## Publishing Poolster's npm packages

`.github/workflows/npm-publish.yml` is the release workflow. Pushing a version
tag such as `v0.5.0` builds each native package on its target platform, then
publishes CLI and SDK native packages before `poolster` and `@relevate/poolster`
after the release verification jobs pass. Manual retries must select a version tag.
The SDK addon is only installed with `@relevate/poolster`; CLI-only installs use
`poolster` and do not download it.

The workflow uses npm trusted publishing via GitHub Actions OIDC. Configure the
same `npm-publish.yml` workflow name as a trusted publisher for every npm
package it publishes. It needs no `NPM_TOKEN`; only its final publish job has
`id-token: write`. Keep that permission out of normal CI and build jobs.
The new `poolster` and `@relevate/poolster-node-<platform>` names need an initial
publication by an authorized maintainer before npm trusted publishing can be
configured for them.

The tag version must match `packages/npm/cli/package.json` and
`packages/npm/sdk/package.json`. Publish a
release only after the tag is protected and the trusted-publisher relationship
has been configured in npm.

## Generated SDK repositories

Optional `poolster sdk init` scaffolds build/test and release workflows from
plugin-emitted package metadata. See [SDK automation](sdk-automation.md) for
independent versions, diff sizing, custom registries, repository synchronization,
and required repository setup. Review with `--dry-run` before writing the files.
