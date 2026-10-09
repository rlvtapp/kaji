# Poolster for Node.js

Embed Poolster's Rust SDK renderers in a Node.js build, with input, output, and
JavaScript plugins selected explicitly in a JavaScript config file. Installing
a plugin does not enable it; add its export to the config.

```sh
npm install -D @relevate/poolster @relevate/poolster-plugin-typescript
```

The native command-line package is `poolster`. Install it separately if you use
the `poolster` command; the Node SDK package does not download that binary.

Create `poolster.config.mjs`:

```js
import { defineConfig, definePlugin } from '@relevate/poolster';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';

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

With the separately installed `poolster` CLI, run `npx poolster generate`. The CLI discovers `poolster.config.mjs`,
`poolster.config.js`, or `poolster.config.cjs` in the current directory. Use `--config`
for another path, `--check` to fail when generated files differ, or `--dry-run`
to inspect changes without writing. Relative input, output and compiler paths
in a config file resolve from the config file's directory.

For programmatic embedding, use `loadConfig` and `createPoolster`:

```js
import { createPoolster, loadConfig } from '@relevate/poolster';

const config = await loadConfig('./poolster.config.mjs');
const result = await createPoolster(config).generate({ write: false });
console.log(result.changes, result.files);
```

`generate(config, { write: false })` is also available directly. Passing an
in-memory config uses paths relative to the process working directory.
`input: { artifacts: './.poolster/openapi' }` reuses artifacts from
`poolster-openapi`. For source OpenAPI input, the published package uses the
compiler from `@relevate/poolster`; consumers do not need Go or Rust toolchains.
Set `compiler` or `POOLSTER_OPENAPI_BIN` to override it.

## Input plugins

Five Rust input providers are available as individual npm packages and from
`@relevate/poolster-plugins`: `inputGraphql`, `inputAsyncApi`, `inputArazzo`,
`inputProtobuf`, and `inputCapnProto`. Select one explicitly:

```js
import { defineConfig, definePlugin } from '@relevate/poolster';
import { inputGraphql } from '@relevate/poolster/plugins';

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
`inspectInput(config.input)` or `createPoolster(config).inspectInput()` returns the
provider, source, summary, and diagnostics without generating files. Native
GraphQL, event, workflow, and RPC contracts do not currently publish Poolster's
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
import { defineInputPlugin } from '@relevate/poolster';

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
| `@relevate/poolster-plugin-typescript` | `pluginTypeScript` |
| `@relevate/poolster-plugin-rust` | `pluginRust` |
| `@relevate/poolster-plugin-go` | `pluginGo` |
| `@relevate/poolster-plugin-python` | `pluginPython` |
| `@relevate/poolster-plugin-php` | `pluginPhp` |
| `@relevate/poolster-plugin-java` | `pluginJava` |
| `@relevate/poolster-plugin-csharp` | `pluginCSharp` |
| `@relevate/poolster-plugin-elixir` | `pluginElixir` |
| `@relevate/poolster-plugin-ruby` | `pluginRuby` |
| `@relevate/poolster-plugin-swift` | `pluginSwift` |

`@relevate/poolster/plugins` exports all language, Rust auxiliary, and input
factories from the main package. The separate `@relevate/poolster-plugins` bundle
and individual plugin packages are also available. Language factories
accept `path`, `name`, `version`, and `style` options. TypeScript also accepts
`transport` (`fetch` or `axios`), `clientName`, and `raw`; Go accepts `jobs`.
The native addon contains the renderers; language packages provide the
separate, typed JavaScript entry points.

## Rust plugins from JavaScript

Compiled Rust plugins exposed by the addon can be selected in the same config:

```js
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';
import { pluginZod } from '@relevate/poolster-plugin-zod';
import { pluginReactQuery } from '@relevate/poolster-plugin-react-query';

plugins: [
  pluginTypeScript({ path: 'web' }),
  pluginZod({ target: 'web', output: 'validation' }),
  pluginReactQuery({ target: 'web', output: 'queries' }),
]
```

The addon currently registers TypeScript's `zod`, `faker`, `msw`, `cypress`,
`react-query`, `vue-query`, and `swr` Rust plugins. Each has an individual
`@relevate/poolster-plugin-<name>` package and an export in the bundle. Pass
`target` to select the TypeScript package path. Call `availableNativePlugins()`
to inspect this addon's registry. The Rust provider graph runs normally inside
that TypeScript package. An arbitrary Rust crate must be linked and registered
when building an addon; installing a crate or npm wrapper alone cannot load it
into a prebuilt native binary.

To expose another Rust plugin, link its crate into the addon, add its
constructor and option mapping to the native registry in
`crates/node/src/lib.rs`, then publish a small npm factory package for
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

Plugins can declare `requires: ['other-plugin-name']`. Poolster orders JavaScript
plugins by those dependencies, rejects missing dependencies and cycles, then
runs each phase in that order. Native SDK entries render together between the
transform and file generation phases. Duplicate output paths fail. `replaceFile`
keeps file ownership and create-once metadata. Emitted files have a stable
owner based on the plugin name, so keep that name stable across releases. Poolster
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

This supports custom JavaScript generators around Poolster's normalized API and
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
(cd openapi && go build -o ../target/debug/poolster-openapi .)
node packages/npm/sdk/scripts/build-native.mjs
node packages/npm/generate-plugins.mjs --check
npm test --prefix packages/npm/sdk
```

The packages are prepared in this repository; npm installation requires
publishing them first. Initial native targets are macOS ARM64/x64, Linux x64
glibc, and Windows x64.

### Native GraphQL generation

The existing TypeScript plugin also generates GraphQL packages. Supply a schema and operation documents:

```js
import { defineConfig } from '@relevate/poolster';
import { inputGraphql } from '@relevate/poolster-input-graphql';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';
export default defineConfig({
  input: { path: './schema.graphql', plugin: inputGraphql(),
    operations: ['./operations.graphql'] },
  output: './generated',
  plugins: [pluginTypeScript({ path: 'client', style: 'flat',
    scalars: { DateTime: { input: 'string', output: 'string' } } })],
});
```

Loaded config files resolve operation paths beside the config. Direct API calls resolve paths from the current directory. Custom scalar mappings describe input and result wire types; they do not convert values at runtime. Unmapped scalars use `unknown`. Subscriptions require `input.subscriptions: true` and an injected subscription transport. `clientName` and Go worker options are rejected for GraphQL. JavaScript hooks receive the input inspection report and emitted files; this API does not expose native typed graph hooks. Unsupported language packages appear in `result.skipped`, warn, and preserve existing owned outputs.

Rust GraphQL output uses the existing `pluginRust()` package. Unmapped custom scalars use `serde_json::Value`. Configure Rust wire types independently with `input.rustScalars`, for example `{ DateTime: { input: 'String', output: 'String' } }`. Mixed generation can also set TypeScript `input.scalars`; each generator uses its own map. Supported Rust mappings are self-contained primitive/container wire types and do not perform runtime conversion. Subscriptions and transport options remain unsupported.

GraphQL output options belong to each language plugin: `pluginTypeScript({ scalars, style: 'flat' })` and `pluginRust({ scalars, style: 'idiomatic' })`. Styles are `raw`, `flat`, and `idiomatic` (`namespaced` is an alias); `raw: true` is an alternative to `style: 'raw'` and cannot be combined with an explicit style. Raw operation exports remain available with each client surface. Idiomatic clients group operations by query/mutation/subscription. Customize resource groups with `groups: { user: { read: 'ReadUser', rename: 'RenameUser' } }`; unassigned operations retain their operation-kind group. Groups require idiomatic/namespaced style.

`input.scalars` and `input.rustScalars` remain compatibility aliases. A mapping supplied both on input and output is accepted when its input/output values are identical; conflicting values are rejected. GraphQL scalar/group options are rejected for OpenAPI generation.

For configurations reused across inputs, scope options to their bundled exporter:

```js
pluginTypeScript({
  contracts: {
    http: { style: 'flat' },
    graphql: { style: 'grouped',
      scalars: { DateTime: { input: 'string', output: 'string' } },
      groups: { user: { read: 'ReadUser' } } },
  },
});
```

The selected input activates `http` or `graphql`; the other block remains independent. `grouped` aliases GraphQL idiomatic/namespaced style. Top-level output options remain shorthand for the active exporter. Equal duplicated options are accepted; conflicting values are rejected. `contracts` currently configures these bundled exporters and does not register arbitrary JavaScript exporters. Each generation uses one input contract; this configuration does not promise combined HTTP/GraphQL Rust packages.

GraphQL packages can attach the existing React Query, Vue Query, SWR, Zod, Faker, MSW and Cypress plugins with their usual `target` and `output` options. For example, `pluginCypress({ target: 'client', cypressOptions: { includeMutations: true, baseUrl: 'http://localhost:4000/graphql' } })` enables mutation smoke helpers explicitly. `pluginFaker({ target: 'client', fixtureOptions: { seed: 42 } })` configures deterministic native fixtures. GraphQL fixture options are supported on Faker; Cypress HTTP operation overrides and nondefault fixture options on other GraphQL addons are rejected. The package compiler requires each emitted integration's upstream runtime/type dependencies.
