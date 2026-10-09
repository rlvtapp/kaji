# Write an input plugin

← [JavaScript plugins](README.md)

Return an inspection summary and the data your output plugins need.

```js
import { readFile } from 'node:fs/promises';
import { defineInputPlugin } from '@relevate/poolster';

export const inputCatalog = defineInputPlugin(() => ({
  kind: 'js-input',
  name: 'example.catalog',
  format: 'catalog',
  async load(source) {
    const data = JSON.parse(await readFile(source, 'utf8'));
    if (typeof data.name !== 'string' || !Array.isArray(data.items)) {
      throw new Error('Catalog requires a name and an items array');
    }
    return {
      summary: {
        format: 'catalog', title: data.name, version: null,
        types: [], operations: [],
      },
      data,
    };
  },
}));
```

Create `catalog.json`:

```json
{ "name": "Product catalog", "items": ["Coffee", "Tea"] }
```

Select `input: { path: './catalog.json', plugin: inputCatalog() }`.
JavaScript outputs read the value through `ctx.input.data`.

Returning `data` does not make existing native SDK generators understand a new
protocol. A provider may additionally return a normalized HTTP `api` and
`securitySchemes` when its semantics really are HTTP.

**Next:** [Share a JavaScript contract](contracts.md) ·
[Input interface types](../../../packages/npm/sdk/index.d.ts)

## Connect an output

Create a config that uses the input and a consumer together:

```js
import { defineConfig, definePlugin } from '@relevate/poolster';
import { inputCatalog } from './input-catalog.mjs';

const report = definePlugin(() => ({
  name: 'example.catalog-report',
  generate(ctx) {
    const catalog = ctx.input?.data;
    if (!catalog || !Array.isArray(catalog.items)) {
      throw new Error('example.catalog-report requires catalog input');
    }
    ctx.emitFile({
      path: 'catalog.txt',
      contents: catalog.items.join('\n') + '\n',
    });
  },
}));

export default defineConfig({
  input: { path: './catalog.json', plugin: inputCatalog() },
  output: './generated',
  plugins: [report()],
});
```

Save the loader above as `input-catalog.mjs` and the config as
`poolster.config.mjs`. Run it with the `generate.mjs` runner from the
[output tutorial](output.md#connect-it-to-an-input). The result is
`generated/catalog.txt`, containing one item per line.

`load` may be asynchronous. Throw an error for an unusable source; return
`diagnostics` for observations a caller can inspect in the report. The summary
is an inspection view, while `data` is the value your consumers use. Neither
implies that a bundled output can generate a client for your format.

## Inspect and test the input separately

```js
import assert from 'node:assert/strict';
import { inspectInput } from '@relevate/poolster';
import { inputCatalog } from './input-catalog.mjs';

const report = await inspectInput({
  path: './catalog.json', plugin: inputCatalog(),
});
assert.equal(report.summary.title, 'Product catalog');
assert.deepEqual(report.data.items, ['Coffee', 'Tea']);
```

Test the loader with invalid JSON and missing fields, then run the input and
output together with `{ write: false }`. Also test a consumer against a different
input: it should give a useful diagnostic rather than dereference unrelated data.

## Adapt an HTTP source

An HTTP adapter can return `api` with `name`, `version`, `operations`, `schemas`
and `annotations`, plus `securitySchemes` as needed. That enables the existing
HTTP renderers. Use the [API declarations](../../../packages/npm/sdk/index.d.ts)
and test generated packages; producing a structurally valid object alone does
not prove that security, presence, nullability or response semantics are correct.

For non-HTTP formats, keep the native data and write a matching consumer. Share
reusable derived data between JavaScript output plugins using
[contract tokens](contracts.md). A JavaScript input does not directly publish
Rust typed contracts or Rust block collections.
