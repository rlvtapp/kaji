# Node embedding and JavaScript plugins

This example uses the in-repository Node API and separate language plugin
packages. It shows a file-based JavaScript config, a TypeScript SDK plus a
reusable documentation plugin, a pre-render API transform, and an artifact-only
preview.

From the repository root, build the native addon and OpenAPI compiler once:

```sh
(cd openapi && go build -o ../target/debug/poolster-openapi .)
node packages/cli/sdk/scripts/build-native.mjs
```

Run the examples:

```sh
POOLSTER_SDK_PACKAGE="$PWD/packages/cli/sdk" node packages/cli/bin/poolster.cjs generate --config examples/node-embedded/poolster.config.mjs
node examples/node-embedded/basic.mjs
node examples/node-embedded/transform.mjs
node examples/node-embedded/native-contracts.mjs
node examples/node-embedded/input-plugins.mjs
POOLSTER_SDK_PACKAGE="$PWD/packages/cli/sdk" node packages/cli/bin/poolster.cjs generate --config examples/node-embedded/poolster.inputs.config.mjs --dry-run
(cd openapi && go run . --out ../examples/node-embedded/.poolster/openapi ../examples/node-embedded/openapi.yaml)
node examples/node-embedded/artifacts.mjs examples/node-embedded/.poolster/openapi
```

`poolster.config.mjs` writes `generated-from-config/typescript` and its catalog.
`basic.mjs` writes `generated/typescript` and `generated/catalog`.
`transform.mjs` removes `/internal/*` operations before the Rust Python SDK
renderer runs, then writes a matching `generated-filtered/public-api` catalog.
`artifacts.mjs` reuses compiler artifacts and previews a Rust SDK without
writing output. Set `POOLSTER_EXAMPLE_OUTPUT` to change the output directory for
each script. The catalog plugin factory is in `plugins/catalog.mjs`. The
TypeScript and Rust plugins come from individual packages; `transform.mjs`
imports the Python plugin from the bundle package.

`native-contracts.mjs` explicitly selects the compiled Rust Zod and React
Query plugins for a TypeScript package. A JS provider inspects their generated
files and publishes a JS contract; a JS post consumer reads that contract and
writes `generated-native/native-report.json`. Rust contract values do not cross
into JS automatically; this example uses the public generated files.

`poolster.inputs.config.mjs` selects the Rust GraphQL parser from its individual
input package and feeds its summary to a JS output plugin. `input-plugins.mjs`
also shows a fully JavaScript input parser with custom data for a JS generator.
These native format inputs do not publish Poolster's HTTP API, so the existing SDK
renderers are not selected for them.
