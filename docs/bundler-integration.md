# Bundler integration

`@relevate/unplugin-poolster` runs the installed Poolster CLI as a build step, so a
Vite/Rollup/Webpack/esbuild/Rspack/Rolldown/Farm project can import generated SDK code without a
separate generation command. It is a trigger for the native generator, not a
JavaScript implementation of OpenAPI generation and not a virtual module.

Install the generator, the integration package, and `unplugin` in the project:

```sh
npm install --save-dev poolster @relevate/unplugin-poolster unplugin
```

For Vite:

```ts
import { defineConfig } from 'vite';
import poolster from '@relevate/unplugin-poolster/vite';

export default defineConfig({
  plugins: [poolster({ config: 'poolster.json' })],
});
```

Use `@relevate/unplugin-poolster/rollup`, `/webpack`, `/esbuild`, `/rspack`,
`/rolldown`, or `/farm` with the corresponding bundler. Every adapter runs Poolster
before the build. During watch or development mode, changing `poolster.json` or its
local `openapi.input` reruns the generator before the bundler sees the changed
generated files.

## Nuxt and Astro

Nuxt projects can register Poolster as a module. It works with either Nuxt builder:

```ts
export default defineNuxtConfig({
  modules: [['@relevate/unplugin-poolster/nuxt', { config: 'poolster.json' }]],
});
```

Astro projects register it as an integration. The integration adds Poolster's Vite
adapter to Astro, including the same development watch behaviour:

```ts
import { defineConfig } from 'astro/config';
import poolster from '@relevate/unplugin-poolster/astro';

export default defineConfig({
  integrations: [poolster({ config: 'poolster.json' })],
});
```

The source document's nested `$ref` files cannot be reliably discovered without
parsing every source format. Declare them (and other local generator inputs)
explicitly:

```ts
poolster({
  config: 'api/poolster.json',
  watchFiles: ['api/components/errors.yaml', 'api/components/models.yaml'],
})
```

For direct CLI generation rather than a JSON recipe, pass exact CLI arguments:

```ts
poolster({
  config: false,
  args: ['generate', 'openapi.yaml', '--output', 'src/generated', '--language', 'typescript'],
})
```

Set `watch: false` to skip follow-up regeneration while retaining the initial
build-step generation. `command` overrides the resolved `poolster`
launcher with a Poolster executable for source builds or unusual deployments.

This package is released separately from `poolster`; it has optional peer
dependencies on `unplugin` for bundler adapters and `@nuxt/kit` for the Nuxt
module. Astro uses the `unplugin` Vite adapter and needs no Astro runtime
dependency from this package.
