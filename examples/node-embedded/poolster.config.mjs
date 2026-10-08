import { defineConfig } from '../../packages/cli/sdk/index.mjs';
import { pluginTypeScript } from '../../packages/node-plugins/typescript/index.mjs';
import { pluginCatalog } from './plugins/catalog.mjs';

export default defineConfig({
  input: './openapi.yaml',
  output: { path: './generated-from-config' },
  name: 'Widgets',
  version: '1.0.0',
  plugins: [
    pluginTypeScript({ name: '@example/widgets' }),
    pluginCatalog(),
  ],
});
