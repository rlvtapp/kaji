# Write an output plugin

← [JavaScript plugins](README.md)

Create `plugin-summary.mjs`:

```js
import { definePlugin } from '@relevate/poolster';

export const pluginSummary = definePlugin(() => ({
  name: 'example.summary',
  generate(ctx) {
    ctx.emitFile({
      path: 'summary.json',
      contents: JSON.stringify(ctx.input?.summary ?? {
        name: ctx.api.name,
        operations: ctx.api.operations.length,
      }, null, 2),
    });
  },
}));
```

Add `pluginSummary()` to the config's `plugins` array. `generate` runs once;
`ctx.emitFile` writes a path relative to the configured output directory.

## Visit HTTP items

```js
hooks: {
  operation(operation, ctx) {
    ctx.emitFile({
      path: `routes/${operation.id}.md`,
      contents: `${operation.method} ${operation.path}
`,
    });
  },
}
```

`schema` and `operation` visit the normalized HTTP API. Native GraphQL inputs do
not invoke these HTTP handlers. Use `ctx.input` for the inspection report and
custom input data, or use a registered GraphQL output plugin.

Use a stable plugin name: it identifies emitted-file ownership. Duplicate paths
fail; `ctx.replaceFile(path, contents)` intentionally updates an existing staged
file while retaining its ownership.

**Next:** [Contracts and hook order](contracts.md) ·
[Runnable plugins](../../../examples/node-embedded/README.md)
