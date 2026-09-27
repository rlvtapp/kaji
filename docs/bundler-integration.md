# Bundler integration

`@relevate/unplugin-kaji` runs the installed Kaji CLI as a build step, so a
Vite/Rollup/Webpack/esbuild/Rspack/Rolldown/Farm project can import generated SDK code without a
separate generation command. It is a trigger for the native generator, not a
JavaScript implementation of OpenAPI generation and not a virtual module.

Install the generator, the integration package, and `unplugin` in the project:

```sh
npm install --save-dev @relevate/kaji @relevate/unplugin-kaji unplugin
```

For Vite:

```ts
import { defineConfig } from 'vite';
import kaji from '@relevate/unplugin-kaji/vite';

export default defineConfig({
  plugins: [kaji({ config: 'kaji.json' })],
});
```

Use `@relevate/unplugin-kaji/rollup`, `/webpack`, `/esbuild`, `/rspack`,
`/rolldown`, or `/farm` with the corresponding bundler. Every adapter runs Kaji
before the build. During watch or development mode, changing `kaji.json` or its
local `openapi.input` reruns the generator before the bundler sees the changed
generated files.

## Nuxt and Astro

Nuxt projects can register Kaji as a module. It works with either Nuxt builder:

```ts
export default defineNuxtConfig({
  modules: [['@relevate/unplugin-kaji/nuxt', { config: 'kaji.json' }]],
});
```

Astro projects register it as an integration. The integration adds Kaji's Vite
adapter to Astro, including the same development watch behaviour:

```ts
import { defineConfig } from 'astro/config';
import kaji from '@relevate/unplugin-kaji/astro';

export default defineConfig({
  integrations: [kaji({ config: 'kaji.json' })],
});
```

The source document's nested `$ref` files cannot be reliably discovered without
parsing every source format. Declare them (and other local generator inputs)
explicitly:

```ts
kaji({
  config: 'api/kaji.json',
  watchFiles: ['api/components/errors.yaml', 'api/components/models.yaml'],
})
```

For direct CLI generation rather than a JSON recipe, pass exact CLI arguments:

```ts
kaji({
  config: false,
  args: ['generate', 'openapi.yaml', '--output', 'src/generated', '--language', 'typescript'],
})
```

Set `watch: false` to skip follow-up regeneration while retaining the initial
build-step generation. `command` overrides the resolved `@relevate/kaji`
launcher with a Kaji executable for source builds or unusual deployments.

This package is released separately from `@relevate/kaji`; it has optional peer
dependencies on `unplugin` for bundler adapters and `@nuxt/kit` for the Nuxt
module. Astro uses the `unplugin` Vite adapter and needs no Astro runtime
dependency from this package.
