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
