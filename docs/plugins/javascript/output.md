# Build a JavaScript output plugin

← [JavaScript plugins](README.md)

This tutorial creates a plugin that writes an operation inventory. It works with
an OpenAPI input and shows how to give a plugin options, install it in a config,
and inspect its output before writing files.

## Set up

Use the [JavaScript quickstart](../../javascript/quickstart.md) to install
`@relevate/poolster` and place an OpenAPI document at `openapi.yaml`. A custom
JavaScript output plugin does not need a language generator alongside it.

Create `plugin-inventory.mjs`:

```js
import { definePlugin } from '@relevate/poolster';

export const pluginInventory = definePlugin((options = {}) => ({
  name: 'example.inventory',
  generate(ctx) {
    if (!ctx.api) {
      throw new Error('example.inventory requires a normalized HTTP API');
    }
    const inventory = ctx.api.operations.map(operation => ({
      name: operation.id,
      method: operation.method,
      path: operation.path,
    }));
    ctx.emitFile({
      path: options.file ?? 'inventory.json',
      contents: JSON.stringify(inventory, null, 2) + '\n',
    });
  },
}));
```

`definePlugin` preserves the factory signature. Calling `pluginInventory(...)`
creates the plugin instance that goes in a config. The name is its stable
identity for diagnostics, dependencies and file ownership.

## Connect it to an input

Create `poolster.config.mjs`:

```js
import { defineConfig } from '@relevate/poolster';
import { pluginInventory } from './plugin-inventory.mjs';

export default defineConfig({
  name: 'Example API',
  version: '1.0.0',
  input: './openapi.yaml',
  output: './generated',
  plugins: [pluginInventory({ file: 'reports/operations.json' })],
});
```

Create `generate.mjs`, then run `node generate.mjs`:

```js
import { generate, loadConfig } from '@relevate/poolster';

const result = await generate(await loadConfig('./poolster.config.mjs'));
console.log(result.changes);
```

The inventory is written to `generated/reports/operations.json`. Paths passed to
`emitFile` are relative to the configured output directory. Emit through the
context so Poolster can detect collisions and manage regeneration ownership.

## Test without writing

Create `plugin-inventory.test.mjs` and run `node --test`:

```js
import assert from 'node:assert/strict';
import test from 'node:test';
import { generate, loadConfig } from '@relevate/poolster';

test('inventory preserves HTTP operation identities', async () => {
  const config = await loadConfig('./poolster.config.mjs');
  const result = await generate(config, { write: false });
  const file = result.files.find(file => file.path === 'reports/operations.json');
  assert.ok(file);
  const operations = JSON.parse(file.contents);
  assert.ok(operations.length > 0);
  assert.ok(operations.every(operation => operation.name && operation.method));
});
```

This tests the real generation path, including input loading. For a plugin with
protocol-specific behavior, also exercise malformed input and unsupported input
kinds. Keep a small local fixture with known operation IDs for precise assertions.

## Per-item handlers

For a file per HTTP operation, replace `generate` with these handlers inside the
plugin object:

```js
hooks: {
  operation(operation, ctx) {
    const filename = encodeURIComponent(operation.id);
    ctx.emitFile({
      path: `routes/${filename}.md`,
      contents: `${operation.method} ${operation.path}\n`,
    });
  },
}
```

`generate` runs once, followed by `schema` and `operation` handlers for the
normalized HTTP API. These are HTTP-specific callbacks. A GraphQL input does not
invoke them; its inspection data is available through `ctx.input`.

## Edit another plugin's output

A Post plugin can read a staged file with `ctx.readFile(path)` and update it with
`ctx.replaceFile(path, contents)`. Declare its producer in `requires` and set
`phase: 'post'`. Replacement preserves the existing file's ownership. Reading a
file that has not been emitted returns `undefined`; decide explicitly whether
that is optional or an error.

Do not use the filesystem to mutate another plugin's staged output. Two plugins
emitting the same path are a collision, even when their contents happen to match.

## Package your plugin

Export the factory from your package entry point and add `@relevate/poolster` as
a peer dependency with a range you have tested. Publish the token definitions
used by other plugins from a shared entry point too. Consumers should import the
same token object rather than recreate it from its name.

**Next:** [Custom input](input.md) · [Contracts and dependencies](contracts.md) ·
[Runnable examples](../../../examples/node-embedded/README.md)
