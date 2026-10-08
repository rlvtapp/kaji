import { defineConfig } from '../../packages/npm/sdk/index.mjs';
import { pluginTypeScript } from '../../packages/npm/plugins/typescript/index.mjs';
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
