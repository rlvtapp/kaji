# Poolster GitHub Action

Generate SDKs from `poolster.json` in GitHub Actions:

```yaml
- uses: actions/checkout@v4
- uses: rlvtapp/kaji/packages/integrations/github@main
  with:
    config: api/poolster.json
```

Pin a Poolster release tag rather than `main` in production. The action installs
Node 22 and invokes the published `poolster` launcher. Set `version` to
pin the npm package and `working-directory` when the recipe is not at the
repository root.
