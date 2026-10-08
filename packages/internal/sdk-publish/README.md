# SDK publish action

Readable composite action and dependency-free Node helper for optional public npm, PyPI, crates.io and Go publishing. The generator vendors `action.yml` and `publish.mjs` into generated repositories; users can edit these sources and community plugins can supply custom publisher commands.

See [publishing setup and registry behavior](../../../docs/sdk-publishing.md). This package is private and is not itself published.

```sh
npm test --prefix packages/internal/sdk-publish
```

Tests do not upload packages. Set `POOLSTER_TEST_PYTHON` to Python 3.11+ to exercise the real Python TOML and archive metadata reader if the default interpreter is older.
