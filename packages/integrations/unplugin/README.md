# @relevate/unplugin-poolster

Generate Poolster SDKs as part of a Vite, Rollup, webpack, esbuild, Rspack,
Rolldown, Farm, Nuxt, or Astro build. The
plugin runs the installed native `poolster` CLI before each build and
regenerates when the recipe or local OpenAPI source changes.

```sh
npm install --save-dev poolster @relevate/unplugin-poolster unplugin
```

## Vite

```ts
// vite.config.ts
import { defineConfig } from 'vite';
import poolster from '@relevate/unplugin-poolster/vite';

export default defineConfig({
  plugins: [poolster({ config: 'poolster.json' })],
});
```

Use the `/rollup`, `/webpack`, `/esbuild`, `/rspack`, `/rolldown`, or `/farm`
entry point for the equivalent `unplugin` adapter. Nuxt projects can use the
`/nuxt` module and Astro projects can use the `/astro` integration. The root
entry point is a dependency-free Rollup/Vite-style plugin if an adapter is not
needed:

```js
const poolster = require('@relevate/unplugin-poolster');

module.exports = { plugins: [poolster()] };
```

## Options

`config` defaults to `poolster.json`, and generates with
`poolster generate --config poolster.json`. The recipe and its local
`openapi.input` are watched automatically. Include local `$ref` files or any
other contract dependencies with `watchFiles`:

```ts
poolster({
  config: 'api/poolster.json',
  watchFiles: ['api/components/common.yaml'],
})
```

For direct CLI generation, supply exact arguments and disable the implicit
recipe:

```ts
poolster({
  config: false,
  args: ['generate', 'openapi.yaml', '--output', 'src/generated', '--language', 'typescript'],
})
```

Other options are `cwd`, `env`, `watch` (defaults to `true`), `silent`, and
`onGenerate`. `command` can point to a local Poolster executable; by default the
plugin resolves the `poolster` launcher from the consuming project.

The plugin never serves generated code as a virtual module. Point normal source
imports at Poolster's configured output directory. Remote OpenAPI URLs are generated
during the initial build but are not polled; use a local downloaded contract if
watch-mode refreshes are needed.

## Development

```sh
node --test packages/integrations/unplugin/test/*.test.cjs
```
