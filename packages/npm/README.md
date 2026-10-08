# npm package workspace

`cli/` and `sdk/` are separate installs. `plugins/` contains individual output
plugin factories, `inputs/` contains input factories, and `plugins-all/` is the
optional bundle. Installing a factory never activates it in a Poolster config.

`generate-plugins.mjs` is the shared source for the factory packages and the
SDK's `plugins.cjs`, `plugins.mjs`, and `plugins.d.ts` convenience exports. It
reads the SDK version from `sdk/package.json`. Changes to a factory or its
types belong in the generator; run `node packages/npm/generate-plugins.mjs`
after editing and `node packages/npm/generate-plugins.mjs --check` in CI.

The SDK owns its runtime and plugin engine. This generator owns only those
three generated convenience export files inside `sdk/`.
