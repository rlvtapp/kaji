import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createPoolster, defineConfig } from '../../packages/npm/sdk/index.mjs';
import { pluginTypeScript } from '../../packages/npm/plugins/typescript/index.mjs';
import { pluginCatalog } from './plugins/catalog.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const output = path.resolve(process.env.POOLSTER_EXAMPLE_OUTPUT ?? path.join(here, 'generated'));
const config = defineConfig({
  input: path.join(here, 'openapi.yaml'),
  output: { path: output },
  name: 'Widgets',
  version: '1.0.0',
  plugins: [
    pluginTypeScript({ path: 'typescript', name: '@example/widgets', transport: 'fetch' }),
    pluginCatalog(),
  ],
});

const result = await createPoolster(config).generate();
console.log(`Wrote ${result.files.length} files to ${result.output}`);
