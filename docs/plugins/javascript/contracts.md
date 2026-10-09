# JavaScript contracts and hooks

← [JavaScript plugins](README.md)

Export one token and share it between producer and consumer.

```js
import { defineContract, definePlugin, requireContract } from '@relevate/poolster';

export const Inventory = defineContract('example.inventory.v1');

export const pluginInventory = definePlugin(() => ({
  name: 'example.inventory',
  provides: [Inventory],
  generate(ctx) {
    ctx.publish(Inventory, { names: ctx.api.schemas.map(s => s.name) });
  },
}));

export const pluginReport = definePlugin(() => ({
  name: 'example.report',
  requires: [requireContract(Inventory)],
  generate(ctx) {
    ctx.emitFile({ path: 'models.json',
      contents: JSON.stringify(ctx.inputs.get(Inventory).names) });
  },
}));
```

Token identity is the exported object. Use `providerHandle(provider, token)`
when several plugins publish that token. Optional requirements use
`requireContract(token, { optional: true })`.

## Available hooks

| Hook | Runs |
| --- | --- |
| `transformApi` | Before native HTTP rendering |
| `generate` | Once after native rendering |
| `schema` / `operation` | Per normalized HTTP item after `generate` |
| `phase: 'post'` | After JavaScript Generate plugins |

Use `requires: ['plugin-name']` for ordering without a data contract.
Dependencies determine order; cycles, ambiguous providers and undeclared access
fail. Every declared contract must be published.

JavaScript contracts are process-local. They do not automatically expose Rust
contracts or native building blocks. JavaScript `onContract`/`onBlock` handlers
are not currently implemented.

**Next:** [Lifecycle diagrams](../../internals/lifecycle.md) ·
[JavaScript API declarations](../../../packages/npm/sdk/index.d.ts)
