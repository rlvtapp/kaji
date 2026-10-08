import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  defineConfig, defineContract, definePlugin, generate,
  providerHandle, requireContract,
} from '../../packages/npm/sdk/index.mjs';
import { pluginTypeScript } from '../../packages/npm/plugins/typescript/index.mjs';
import { pluginZod } from '../../packages/npm/plugins/zod/index.mjs';
import { pluginReactQuery } from '../../packages/npm/plugins/react-query/index.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const NativeFiles = defineContract('example.native-files');

const pluginInspectNative = definePlugin(() => ({
  name: 'inspect-native',
  provides: [NativeFiles],
  generate(ctx) {
    const validation = ctx.readFile('web/validation.ts');
    const queries = ctx.readFile('web/queries.ts');
    ctx.publish(NativeFiles, {
      validation: typeof validation === 'string',
      queries: typeof queries === 'string',
    });
  },
}));

const inspect = pluginInspectNative();
const pluginReport = definePlugin(() => ({
  name: 'native-report',
  phase: 'post',
  requires: [requireContract(NativeFiles, { from: providerHandle(inspect, NativeFiles) })],
  generate(ctx) {
    const found = ctx.inputs.get(NativeFiles);
    ctx.emitFile({
      path: 'native-report.json',
      contents: `${JSON.stringify(found, null, 2)}\n`,
    });
  },
}));

const result = await generate(defineConfig({
  input: path.join(here, 'openapi.yaml'),
  output: path.resolve(process.env.POOLSTER_EXAMPLE_OUTPUT ?? path.join(here, 'generated-native')),
  name: 'Widgets',
  version: '1.0.0',
  plugins: [
    pluginTypeScript({ path: 'web' }),
    pluginZod({ target: 'web', output: 'validation' }),
    pluginReactQuery({ target: 'web', output: 'queries' }),
    pluginReport(),
    inspect,
  ],
}));

console.log(`Wrote ${result.files.length} files, including Rust plugin output and a JS post-plugin report.`);
