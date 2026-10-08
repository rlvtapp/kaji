# Editable SDK language checks

Use this composite action after checkout, or let `poolster sdk init` copy `action.yml`
and `check.mjs` into `.github/actions/poolster-check` in the generated repository.

```yaml
permissions:
  contents: read
jobs:
  sdk:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          persist-credentials: false
      - uses: rlvtapp/kaji/packages/internal/sdk-check@YOUR_PINNED_REVISION
        with:
          path: generated/go
          language: go
```

Built-in setups cover Rust, TypeScript, Go, Python, PHP/Symfony, Java, C#/.NET,
Elixir, Ruby and Swift. Swift jobs use macOS; the generated matrix sets this
runner. Metadata with explicit nonempty `build` and `test` argument vectors wins
over native defaults, making checks work with custom plugins and project layouts.
Install a community plugin's custom toolchain in its workflow before this action.

Native defaults run available tests and compile/lint each SDK. They cannot create
missing behavioral tests: PHP/Ruby defaults check syntax, TypeScript builds, and
Swift/Elixir test only when a test directory exists. Declare behavioral test
commands and generate wire fixtures for stronger checks. No credentials/OIDC are
needed for ordinary checks. Dependency scripts are disabled by TypeScript's
fallback; custom commands are trusted repository code.

Modify these sources directly. Generation protects edited vendored files and
asks you to merge updates when the upstream action changes; it does not silently
reset custom checks. `node --test packages/internal/sdk-check/test/*.mjs` verifies literal
arguments, custom metadata, native defaults, failure propagation and path bounds.
