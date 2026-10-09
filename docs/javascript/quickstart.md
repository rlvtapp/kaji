# Generate your first client

← [JavaScript](README.md)

Use Node.js 22 or newer and an OpenAPI document named `openapi.yaml`.

## 1. Install

```sh
npm install -D @relevate/poolster @relevate/poolster-plugin-typescript
```

## 2. Configure

Create `poolster.config.mjs`:

```js
import { defineConfig } from '@relevate/poolster';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';

export default defineConfig({
  input: './openapi.yaml',
  output: './generated',
  plugins: [pluginTypeScript({ path: 'client', name: '@example/client' })],
});
```

## 3. Generate

Create `generate.mjs`:

```js
import { generate, loadConfig } from '@relevate/poolster';

const result = await generate(await loadConfig('./poolster.config.mjs'));
console.log(result.changes);
```

```sh
node generate.mjs
```

The package appears in `generated/client`. Build it using its generated
`package.json`, then import the compiled exports. Generated TypeScript compiles
into JavaScript; your application can use either language.

**Next:** [Add plugins](plugins.md) or [configure another package](configuration.md).

The optional `poolster` CLI is a separate npm package. This quickstart uses only
the Node API and does not require the CLI.
