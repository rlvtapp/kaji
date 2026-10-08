# Kaji for Node.js

Embed Kaji's Rust SDK renderers in a Node.js build, with input, output, and
JavaScript plugins selected explicitly in a JavaScript config file. Installing
a plugin does not enable it; add its export to the config.

```sh
npm install -D @relevate/kaji
```

The native command-line package is `kajicli`. Install it separately if you use
the `kaji` command; the Node SDK package does not download that binary.

Create `kaji.config.mjs`:

```js
import { defineConfig, definePlugin } from '@relevate/kaji/sdk';
import { pluginTypeScript } from '@relevate/kaji/sdk/plugins';

const pluginRoutes = definePlugin(() => ({
  name: 'routes',
  hooks: {
    operation(operation, ctx) {
      ctx.emitFile({
        path: `routes/${operation.id}.md`,
        contents: `# ${operation.method} ${operation.path}\n`,
      });
    },
  },
}));

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated' },
  name: 'Notes',
  version: '1.0.0',
  plugins: [
    pluginTypeScript({ name: '@acme/notes', transport: 'fetch' }),
    pluginRoutes(),
  ],
});
```

Run `npx kaji-sdk generate`. The CLI discovers `kaji.config.mjs`,
`kaji.config.js`, or `kaji.config.cjs` in the current directory. Use `--config`
for another path, `--check` to fail when generated files differ, or `--dry-run`
to inspect changes without writing. Relative input, output and compiler paths
in a config file resolve from the config file's directory.

For programmatic embedding, use `loadConfig` and `createKaji`:

```js
import { createKaji, loadConfig } from '@relevate/kaji/sdk';

const config = await loadConfig('./kaji.config.mjs');
const result = await createKaji(config).generate({ write: false });
console.log(result.changes, result.files);
```

`generate(config, { write: false })` is also available directly. Passing an
in-memory config uses paths relative to the process working directory.
`input: { artifacts: './.kaji/openapi' }` reuses artifacts from
`kaji-openapi`. For source OpenAPI input, the published package uses the
compiler from `@relevate/kaji`; consumers do not need Go or Rust toolchains.
Set `compiler` or `KAJI_OPENAPI_BIN` to override it.

## Input plugins

Five Rust input providers are available as individual npm packages and from
`@relevate/kaji-plugins`: `inputGraphql`, `inputAsyncApi`, `inputArazzo`,
`inputProtobuf`, and `inputCapnProto`. Select one explicitly:

```js
import { defineConfig, definePlugin } from '@relevate/kaji/sdk';
import { inputGraphql } from '@relevate/kaji/sdk/plugins';

const summary = definePlugin(() => ({
  name: 'summary',
  generate(ctx) {
    ctx.emitFile({ path: 'schema.json', contents: JSON.stringify(ctx.input.summary, null, 2) });
  },
}));

export default defineConfig({
  input: { path: './schema.graphql', plugin: inputGraphql() },
  output: './generated',
  plugins: [summary()],
});
```

`availableInputPlugins()` lists the Rust providers compiled into the addon.
`inspectInput(config.input)` or `createKaji(config).inspectInput()` returns the
provider, source, summary, and diagnostics without generating files. Native
GraphQL, event, workflow, and RPC contracts do not currently publish Kaji's
normalized HTTP API, so the existing language SDK plugins cannot consume them.
JavaScript output plugins can use `ctx.input` to generate their own artifacts.
An additional Rust input provider needs to be linked into a custom addon and
registered there; installing an npm factory does not dynamically load a Rust
crate into the prebuilt addon.

Use `defineInputPlugin` to publish a parser from a Node package. Its `load`
method receives an absolute source path and returns a summary, optional
diagnostics and data. If it also returns a normalized `api` and optional
`securitySchemes`, the Rust HTTP SDK renderers can consume that API:

```js
import { readFile } from 'node:fs/promises';
import { defineInputPlugin } from '@relevate/kaji/sdk';

export const inputJsonApi = defineInputPlugin(() => ({
  kind: 'js-input', name: 'example.json-api', format: 'http-json',
  async load(source) {
    const api = JSON.parse(await readFile(source, 'utf8'));
    return {
      summary: { format: 'http-json', title: api.name, version: api.version,
        types: api.schemas.map((schema) => schema.name), operations: [] },
      api,
    };
  },
}));
```

Select it with `input: { path: './api.json', plugin: inputJsonApi() }`. A JS
input can also return `data`; downstream JS plugins receive it as
`ctx.input.data`. See the [mixed input example](../../../examples/node-embedded/README.md).

## Language packages

Install only the languages you use:

| Package | Export |
| --- | --- |
| `@relevate/kaji-plugin-typescript` | `pluginTypeScript` |
| `@relevate/kaji-plugin-rust` | `pluginRust` |
| `@relevate/kaji-plugin-go` | `pluginGo` |
| `@relevate/kaji-plugin-python` | `pluginPython` |
| `@relevate/kaji-plugin-php` | `pluginPhp` |
| `@relevate/kaji-plugin-java` | `pluginJava` |
| `@relevate/kaji-plugin-csharp` | `pluginCSharp` |
| `@relevate/kaji-plugin-elixir` | `pluginElixir` |
| `@relevate/kaji-plugin-ruby` | `pluginRuby` |
| `@relevate/kaji-plugin-swift` | `pluginSwift` |

`@relevate/kaji/sdk/plugins` exports all language, Rust auxiliary, and input
factories from the main package. The separate `@relevate/kaji-plugins` bundle
and individual plugin packages are also available. Language factories
accept `path`, `name`, `version`, and `style` options. TypeScript also accepts
`transport` (`fetch` or `axios`), `clientName`, and `raw`; Go accepts `jobs`.
The native addon contains the renderers; language packages provide the
separate, typed JavaScript entry points.

## Rust plugins from JavaScript

Compiled Rust plugins exposed by the addon can be selected in the same config:

```js
import { pluginTypeScript } from '@relevate/kaji-plugin-typescript';
import { pluginZod } from '@relevate/kaji-plugin-zod';
import { pluginReactQuery } from '@relevate/kaji-plugin-react-query';

plugins: [
  pluginTypeScript({ path: 'web' }),
  pluginZod({ target: 'web', output: 'validation' }),
  pluginReactQuery({ target: 'web', output: 'queries' }),
]
```

The addon currently registers TypeScript's `zod`, `faker`, `msw`, `cypress`,
`react-query`, `vue-query`, and `swr` Rust plugins. Each has an individual
`@relevate/kaji-plugin-<name>` package and an export in the bundle. Pass
`target` to select the TypeScript package path. Call `availableNativePlugins()`
to inspect this addon's registry. The Rust provider graph runs normally inside
that TypeScript package. An arbitrary Rust crate must be linked and registered
when building an addon; installing a crate or npm wrapper alone cannot load it
into a prebuilt native binary.

To expose another Rust plugin, link its crate into the addon, add its
constructor and option mapping to the native registry in
`crates/kaji-node/src/lib.rs`, then publish a small npm factory package for
explicit selection. The `availableNativePlugins()` list and generation tests
should cover that registration. The existing packages are examples of this
build-time registration model.

## JavaScript plugins

`transformApi(api)` runs after OpenAPI normalization and before native SDK
rendering. Mutate the API or return a replacement to change what every renderer
sees. `generate(ctx)` runs once after native rendering. `schema(schema, ctx)`
and `operation(operation, ctx)` run per normalized item. Hooks can be async and
may live in the `hooks` object or directly on the plugin. The context includes
`api`, `securitySchemes`, `output`, `emitFile`, `readFile`, and `replaceFile`.

Plugins can declare `requires: ['other-plugin-name']`. Kaji orders JavaScript
plugins by those dependencies, rejects missing dependencies and cycles, then
runs each phase in that order. Native SDK entries render together between the
transform and file generation phases. Duplicate output paths fail. `replaceFile`
keeps file ownership and create-once metadata. Emitted files have a stable
owner based on the plugin name, so keep that name stable across releases. Kaji
refuses to overwrite locally edited generated files and leaves unrelated files
alone.

For JS-to-JS provider contracts, export a process-local token with
`defineContract<T>('name')`. A plugin declares `provides: [token]` and calls
`ctx.publish(token, value)`. Consumers declare
`requires: [requireContract(token)]`, then use `ctx.inputs.get(token)`.
`providerHandle(provider, token)` selects one of several providers; optional
requirements return `undefined` when absent. The JS engine rejects cycles,
ambiguous or missing providers, undeclared reads/publications, and missing
publications. `phase: 'post'` runs after JS generation plugins. All JS plugins
share `ctx.workspace` during one generation. The implementation is JavaScript
with JSDoc and TypeScript declarations, so publishing a plugin does not need a
TypeScript build step.

This supports custom JavaScript generators around Kaji's normalized API and
explicit selection of registered Rust plugins. JS contracts do not expose
arbitrary Rust `TypeId` values or language workspaces. Native contract values
need a deliberately written adapter before JS can consume them. The JS phases
run after native package finalization, so they are not identical to the Rust
plugin lifecycle. This also does not implement Kubb's parser/resolver ecosystem.

See [the runnable examples](../../../examples/node-embedded/README.md) for a
config file, individual and bundled imports, native Rust auxiliaries, JS
contracts, a custom generator, transforms, and artifact-only previews.

## Development

From the repository root:

```sh
(cd openapi && go build -o ../target/debug/kaji-openapi .)
node packages/cli/sdk/scripts/build-native.mjs
node packages/node-plugins/generate.mjs --check
npm test --prefix packages/cli/sdk
```

The packages are prepared in this repository; npm installation requires
publishing them first. Initial native targets are macOS ARM64/x64, Linux x64
glibc, and Windows x64.
