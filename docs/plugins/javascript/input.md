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
    return {
      summary: {
        format: 'catalog', title: data.name,
        types: [], operations: [],
      },
      data,
    };
  },
}));
```

Select `input: { path: './catalog.json', plugin: inputCatalog() }`.
JavaScript outputs read the value through `ctx.input.data`.

Returning `data` does not make existing native SDK generators understand a new
protocol. A provider may additionally return a normalized HTTP `api` and
`securitySchemes` when its semantics really are HTTP.

**Next:** [Share a JavaScript contract](contracts.md) ·
[Input interface types](../../../packages/npm/sdk/index.d.ts)
