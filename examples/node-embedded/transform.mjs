import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig, definePlugin, generate } from '../../packages/cli/sdk/index.mjs';
import { pluginPython } from '../../packages/node-plugins/all/index.mjs';
import { pluginCatalog } from './plugins/catalog.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const output = path.resolve(process.env.KAJI_EXAMPLE_OUTPUT ?? path.join(here, 'generated-filtered'));
const pluginPublicOnly = definePlugin(() => ({
  name: 'public-only',
  hooks: {
    transformApi(api) {
      return {
        ...api,
        operations: api.operations.filter((operation) => !operation.path.startsWith('/internal/')),
      };
    },
  },
}));

const result = await generate(defineConfig({
  input: path.join(here, 'openapi.yaml'),
  output: { path: output },
  name: 'Public Widgets',
  version: '1.0.0',
  plugins: [
    pluginPublicOnly(),
    pluginPython({ path: 'python', name: 'widgets-sdk' }),
    pluginCatalog({ directory: 'public-api' }),
  ],
}));
console.log(`Wrote ${result.files.length} public API files to ${result.output}`);
