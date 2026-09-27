# Kaji GitLab CI template

Include the template and pin it to a Kaji release tag:

```yaml
include:
  - remote: 'https://raw.githubusercontent.com/rlvtapp/kaji/<kaji-release-tag>/packages/gitlab-ci/kaji.yml'

generate-sdk:
  extends: .kaji:generate
  variables:
    KAJI_CONFIG: api/kaji.json
```

The template uses the Debian-based Node 22 image because Kaji's Linux launcher
requires glibc (Alpine/musl is not supported). Set
`KAJI_VERSION` to an npm version or dist-tag, and `KAJI_WORKING_DIRECTORY` when
the config is outside the repository root.
