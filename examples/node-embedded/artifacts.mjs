import path from 'node:path';
import { defineConfig, generate } from '../../packages/npm/sdk/index.mjs';
import { pluginRust } from '../../packages/npm/plugins/rust/index.mjs';

if (!process.argv[2]) {
  console.error('Usage: node examples/node-embedded/artifacts.mjs <artifact-directory>');
  process.exitCode = 2;
} else {
  const result = await generate(defineConfig({
    input: { artifacts: path.resolve(process.argv[2]) },
    output: path.resolve(process.env.POOLSTER_EXAMPLE_OUTPUT ?? 'generated-from-artifacts'),
    name: 'Widgets',
    version: '1.0.0',
    plugins: [pluginRust({ path: 'rust', name: 'widgets_sdk' })],
  }), { write: false });
  console.log(`Preview: ${result.files.length} files; ${result.changes.added.length} would be added.`);
}
