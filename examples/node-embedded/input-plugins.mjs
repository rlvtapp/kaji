import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createPoolster, defineConfig, defineInputPlugin, definePlugin } from '../../packages/cli/sdk/index.mjs';
import nativeConfig from './poolster.inputs.config.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const native = await createPoolster(nativeConfig).generate({ write: false });
console.log(`Rust GraphQL input exposed ${native.input.summary.types.length} types to a JS output plugin`);

const inputJson = defineInputPlugin(() => ({
  kind: 'js-input',
  name: 'example.json-catalog',
  format: 'json-catalog',
  async load(source) {
    const data = JSON.parse(await fs.readFile(source, 'utf8'));
    return {
      summary: { format: 'json-catalog', title: data.title, version: null,
        types: data.items.map((item) => item.name), operations: [] },
      data,
    };
  },
}));
const pluginCatalog = definePlugin(() => ({
  name: 'catalog-output',
  generate(ctx) {
    ctx.emitFile({ path: 'catalog.md', contents: `# ${ctx.input.data.title}\n\n${ctx.input.data.items.map((item) => `- ${item.name}`).join('\n')}\n` });
  },
}));

const custom = defineConfig({
  input: { path: path.join(here, 'input-catalog.json'), plugin: inputJson() },
  output: path.resolve(process.env.POOLSTER_EXAMPLE_OUTPUT ?? path.join(here, 'generated-json-input')),
  plugins: [pluginCatalog()],
});
const generated = await createPoolster(custom).generate({ write: false });
console.log(generated.files[0].contents);
