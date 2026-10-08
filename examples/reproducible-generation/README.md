# Reproducible, focused generation

This recipe generates TypeScript and C# SDKs from only the public messages
surface. It demonstrates a committed path slice and the generation lock file
that explains exactly what Kaji produced.

```sh
cd examples/reproducible-generation
npx kajicli generate
```

The `openapi.paths` configuration includes `/messages*` and `/admin*`, then
excludes `/admin/audit*`. Exclusions always win, so `listMessages` is the only
operation in the output:

```text
generated/
  .kaji/generation.lock.json
  typescript/
  csharp/
```

Commit the lock file alongside generated code. It records the Kaji version,
source/config hashes, path selection, target settings, and resulting operation
inventory—but never credentials. Review it when the specification or generator
changes.

The direct-command equivalent is useful for one-off work:

```sh
npx kajicli generate openapi.yaml --output generated --language typescript,csharp \
  --include-path '/messages*' --include-path '/admin*' --exclude-path '/admin/audit*'
```

For a public contract you do not yet have locally, first use
[`kaji discover` and `kaji download`](../../docs/discovery.md); private
contracts should use the authenticated remote-input recipe instead.
