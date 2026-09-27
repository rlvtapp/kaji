# @relevate/unplugin-kaji

Generate Kaji SDKs as part of a Vite, Rollup, webpack, esbuild, Rspack,
Rolldown, Farm, Nuxt, or Astro build. The
plugin runs the installed native `@relevate/kaji` CLI before each build and
regenerates when the recipe or local OpenAPI source changes.

```sh
npm install --save-dev @relevate/kaji @relevate/unplugin-kaji unplugin
```

## Vite

```ts
// vite.config.ts
import { defineConfig } from 'vite';
import kaji from '@relevate/unplugin-kaji/vite';

export default defineConfig({
  plugins: [kaji({ config: 'kaji.json' })],
});
```

Use the `/rollup`, `/webpack`, `/esbuild`, `/rspack`, `/rolldown`, or `/farm`
entry point for the equivalent `unplugin` adapter. Nuxt projects can use the
`/nuxt` module and Astro projects can use the `/astro` integration. The root
entry point is a dependency-free Rollup/Vite-style plugin if an adapter is not
needed:

```js
const kaji = require('@relevate/unplugin-kaji');

module.exports = { plugins: [kaji()] };
```

## Options

`config` defaults to `kaji.json`, and generates with
`kaji generate --config kaji.json`. The recipe and its local
`openapi.input` are watched automatically. Include local `$ref` files or any
other contract dependencies with `watchFiles`:

```ts
kaji({
  config: 'api/kaji.json',
  watchFiles: ['api/components/common.yaml'],
})
```

For direct CLI generation, supply exact arguments and disable the implicit
recipe:

```ts
kaji({
  config: false,
  args: ['generate', 'openapi.yaml', '--output', 'src/generated', '--language', 'typescript'],
})
```

Other options are `cwd`, `env`, `watch` (defaults to `true`), `silent`, and
`onGenerate`. `command` can point to a local Kaji executable; by default the
plugin resolves the `@relevate/kaji` launcher from the consuming project.

The plugin never serves generated code as a virtual module. Point normal source
imports at Kaji's configured output directory. Remote OpenAPI URLs are generated
during the initial build but are not polled; use a local downloaded contract if
watch-mode refreshes are needed.

## Development

```sh
node --test packages/unplugin-kaji/test/*.test.cjs
```
