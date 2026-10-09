# JavaScript contracts and hooks

← [JavaScript plugins](README.md)

This HTTP example derives a model inventory. Export one token and share it
between producer and consumer.

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

## Select a provider explicitly

A dependency without a handle works when exactly one plugin provides its token.
When two plugins provide it, select the instance you want:

```js
import { definePlugin, providerHandle, requireContract } from '@relevate/poolster';
import { Inventory, pluginInventory } from './inventory.mjs';

const source = pluginInventory();
const report = definePlugin(() => ({
  name: 'example.selected-report',
  requires: [requireContract(Inventory, {
    from: providerHandle(source, Inventory),
  })],
  generate(ctx) {
    const value = ctx.inputs.get(Inventory);
    ctx.emitFile({ path: 'selected.json', contents: JSON.stringify(value) });
  },
}));

// Put source and report() in the same config's plugins array.
```

The handle points to that particular plugin object. Creating another instance
with the same factory does not register the original instance. The plugin array
can put the consumer first: the graph still runs its selected producer first.

## Optional data

Declare `requireContract(Inventory, { optional: true })`, then read it with
`ctx.inputs.optional(Inventory)`. The result is `undefined` if no provider exists.
Optionality does not resolve ambiguous providers. An undeclared read remains an
error even when a matching provider happens to be installed.

## Transform a published value

A transformer requires the original token from a selected provider and provides
the token for its own result. It reads with `ctx.inputs.get`, builds a new value,
and calls `ctx.publish`. Bind downstream consumers to the transformer's handle.
Do not mutate a shared value in place and expect downstream order to select a
revision automatically.

JavaScript tokens currently carry no native contract revision or block provenance
protocol. The Rust contracts and blocks system has those additional checks.

## Diagnose graph failures

| Diagnostic | Fix |
| --- | --- |
| Missing provider | Install a matching producer or declare the requirement optional |
| Ambiguous provider | Bind `from` to the intended plugin instance |
| Unregistered handle | Add the exact provider object to the config |
| Undeclared read | Add a requirement for the token |
| Missing publication | Publish every declared token on every successful execution |
| Dependency cycle | Separate mutually dependent work into ordered stages |
| Generate depends on Post | Move the consumer to Post or publish earlier |

For ordering without exchanging data, `requires: ['example.inventory']` depends
on a plugin name. Use token requirements when the consumer needs an actual value.

## Post processing

Set `phase: 'post'` on a plugin that edits completed JavaScript Generate output.
Its `generate` callback still runs once, but in the Post phase. Native language
assembly has already happened by the time these JavaScript callbacks run; a
JavaScript file emission does not automatically register exports or dependencies
in the native language workspace.

Use `ctx.workspace` for shared JavaScript state when no typed dependency is needed.
A map entry does not establish graph ordering: declare the producer dependency.
