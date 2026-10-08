import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig, definePlugin } from '../../packages/npm/sdk/index.mjs';
import { inputGraphql } from '../../packages/npm/inputs/graphql/index.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const pluginSchemaReport = definePlugin(() => ({
  name: 'schema-report',
  generate(ctx) {
    ctx.emitFile({
      path: 'schema-summary.json',
      contents: `${JSON.stringify(ctx.input.summary, null, 2)}\n`,
    });
  },
}));

export default defineConfig({
  input: {
    path: path.resolve(here, '../../crates/inputs/graphql/tests/fixtures/github/schema.graphql'),
    plugin: inputGraphql(),
  },
  output: path.resolve(process.env.POOLSTER_EXAMPLE_OUTPUT ?? path.join(here, 'generated-input')),
  plugins: [pluginSchemaReport()],
});
