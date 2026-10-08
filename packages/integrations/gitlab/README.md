# Poolster GitLab CI template

Include the template and pin it to a Poolster release tag:

```yaml
include:
  - remote: 'https://raw.githubusercontent.com/rlvtapp/kaji/<poolster-release-tag>/packages/integrations/gitlab/poolster.yml'

generate-sdk:
  extends: .poolster:generate
  variables:
    POOLSTER_CONFIG: api/poolster.json
```

The template uses the Debian-based Node 22 image because Poolster's Linux launcher
requires glibc (Alpine/musl is not supported). Set
`POOLSTER_VERSION` to an npm version or dist-tag, and `POOLSTER_WORKING_DIRECTORY` when
the config is outside the repository root.
