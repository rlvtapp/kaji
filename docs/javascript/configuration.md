# Configure generation

← [JavaScript](README.md)

Use `poolster.config.mjs` and export `defineConfig(...)`.

```js
export default defineConfig({
  input: './openapi.yaml',
  output: './generated',
  name: 'Example API',
  version: '1.0.0',
  plugins: [
    pluginTypeScript({ path: 'web', name: '@example/web' }),
    pluginRust({ path: 'rust', name: 'example-client' }),
  ],
});
```

Import each factory from its plugin package. Installing a plugin does not enable it.
Loaded config paths resolve from the config directory; in-memory configs use the
current working directory.

## Options per contract

```js
pluginTypeScript({
  contracts: {
    http: { style: 'flat', transport: 'fetch' },
    graphql: { style: 'flat' },
  },
});
```

The selected input activates its block. Top-level options remain shorthand;
conflicting duplicated options fail. Bundled exporter keys are currently `http`
and `graphql`; this object does not register a custom contract.

## Other inputs

```js
input: {
  path: './schema.graphql',
  plugin: inputGraphql(),
  operations: ['./operations.graphql'],
}
```

A supported input needs a compatible output. Unsupported GraphQL language outputs
are reported in `result.skipped` with a warning; existing owned files are preserved.
Invalid supported inputs and missing required plugin dependencies still fail.

**Next:** [GraphQL](graphql.md) · [All option types](../../packages/npm/sdk/index.d.ts)
