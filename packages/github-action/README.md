# Kaji GitHub Action

Generate SDKs from `kaji.json` in GitHub Actions:

```yaml
- uses: actions/checkout@v4
- uses: rlvtapp/kaji/packages/github-action@main
  with:
    config: api/kaji.json
```

Pin a Kaji release tag rather than `main` in production. The action installs
Node 22 and invokes the published `@relevate/kaji` launcher. Set `version` to
pin the npm package and `working-directory` when the recipe is not at the
repository root.
