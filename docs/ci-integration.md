# CI integration

Kaji ships repository-owned integrations for GitHub Actions and GitLab CI. Both
install the published npm launcher, so CI needs Node but not Rust or Go.

## GitHub Actions

```yaml
steps:
  - uses: actions/checkout@v4
  - uses: rlvtapp/kaji/packages/github-action@main
    with:
      config: api/kaji.json
```

The action accepts `config`, `version`, and `working-directory`. Pin the action
and npm `version` to a release in production. See the
[action README](../packages/github-action/README.md) for all inputs.

## GitLab CI

```yaml
include:
  - remote: 'https://raw.githubusercontent.com/rlvtapp/kaji/<kaji-release-tag>/packages/gitlab-ci/kaji.yml'

generate-sdk:
  extends: .kaji:generate
  variables:
    KAJI_CONFIG: api/kaji.json
```

The template accepts `KAJI_CONFIG`, `KAJI_VERSION`, and
`KAJI_WORKING_DIRECTORY`. It intentionally uses a Debian-based Node image:
Kaji's published Linux launcher requires glibc and therefore does not run on
Alpine/musl. See the
[template README](../packages/gitlab-ci/README.md) for details.

## GitHub Marketplace

The repository-local action can be used directly, but it cannot be listed in
GitHub Marketplace: Marketplace actions require one `action.yml` at the root of
a public action repository. When Kaji is ready to publish a Marketplace action,
create a small dedicated public repository (for example `rlvtapp/kaji-action`)
whose root contains this action, release it under a stable tag, and publish that
release from GitHub. Do not add a root action metadata file to this monorepo
solely for Marketplace discovery.

## Publishing Kaji's npm packages

`.github/workflows/npm-publish.yml` is the release workflow. Pushing a version
tag such as `v0.1.0` builds each native package on its target platform, then
publishes the four native packages before `@relevate/kaji` and the optional
unscoped `kaji` facade.

The workflow uses npm trusted publishing via GitHub Actions OIDC. Configure the
same `npm-publish.yml` workflow name as a trusted publisher for every npm
package it publishes. It needs no `NPM_TOKEN`; only its final publish job has
`id-token: write`. Keep that permission out of normal CI and build jobs.

The tag version must exactly match the versions in `packages/cli/package.json`
and `packages/npm/package.json`. Publish a release only after the tag is
protected and the trusted-publisher relationship has been configured in npm.
