'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const test = require('node:test');

const { availableNativePlugins, generate } = require('../index.cjs');
const bundle = require('../../plugins-all/index.cjs');
const { pluginTypeScript } = require('../../plugins/typescript/index.cjs');
const { pluginZod } = require('../../plugins/zod/index.cjs');
const { pluginReactQuery } = require('../../plugins/react-query/index.cjs');
const { artifacts, config, temporary } = require('../test-support/helpers.cjs');

test('JS config can select compiled Rust plugins within a native SDK package', async (t) => {
  assert.deepEqual(availableNativePlugins(), [
    'typescript/zod', 'typescript/faker', 'typescript/msw', 'typescript/cypress',
    'typescript/react-query', 'typescript/vue-query', 'typescript/swr',
  ]);
  const compiled = await artifacts(t);
  const dir = await temporary(t);

  await t.test('individual and bundled factories select the same Rust plugins', async () => {
    assert.deepEqual(pluginZod({ target: 'web', output: 'validation' }), bundle.pluginZod({ target: 'web', output: 'validation' }));
    assert.deepEqual(pluginReactQuery({ target: 'web', output: 'queries' }), bundle.pluginReactQuery({ target: 'web', output: 'queries' }));
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'native'), [
      pluginTypeScript({ path: 'web' }),
      pluginZod({ target: 'web', output: 'validation' }),
      pluginReactQuery({ target: 'web', output: 'queries' }),
    ]), { write: false });
    assert.ok(result.files.some((file) => file.path === 'web/validation.ts' && /zod/i.test(file.contents)));
    assert.ok(result.files.some((file) => file.path === 'web/queries.ts' && /query/i.test(file.contents)));
  });

  await t.test('all seven registered Rust add-ons can render in one package', async () => {
    const factories = [
      ['zod', bundle.pluginZod], ['faker', bundle.pluginFaker],
      ['msw', bundle.pluginMsw], ['cypress', bundle.pluginCypress],
      ['react-query', bundle.pluginReactQuery], ['vue-query', bundle.pluginVueQuery], ['swr', bundle.pluginSwr],
    ];
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'all-native'), [
      bundle.pluginTypeScript({ path: 'web' }),
      ...factories.map(([id, factory]) => factory({ target: 'web', output: `extensions/${id}` })),
    ]), { write: false });
    for (const [id] of factories) {
      assert.ok(result.files.some((file) => file.path.startsWith(`web/extensions/${id}`)), `${id} did not render`);
    }
  });

  await t.test('target and duplicate selection fail before writing', async () => {
    assert.throws(() => pluginZod({ output: '' }), /output must be a nonempty string/);
    assert.throws(() => bundle.pluginZod({ target: '' }), /target must be a nonempty string/);
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'missing-target'), [
      pluginTypeScript({ path: 'web' }), pluginZod({ target: 'missing' }),
    ])), /needs exactly one SDK package at missing/);
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'duplicate'), [
      pluginTypeScript({ path: 'web' }), pluginZod({ target: 'web' }), bundle.pluginZod({ target: 'web' }),
    ])), /configured twice/);
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'unregistered'), [
      pluginTypeScript({ path: 'web' }),
      { kind: 'native-addon', name: 'unknown', plugin: 'unknown', target: 'web' },
    ])), /not registered in this addon/);
  });

  await t.test('a Rust plugin rejects an escaping output path without writing', async () => {
    const output = path.join(dir, 'unsafe-native-output');
    await assert.rejects(generate(config({ artifacts: compiled }, output, [
      pluginTypeScript({ path: 'web' }), pluginZod({ target: 'web', output: '../escape' }),
    ])), /generated paths must be relative and cannot escape/);
    await assert.rejects(fs.stat(output), { code: 'ENOENT' });
    await assert.rejects(fs.stat(path.join(dir, 'escape.ts')), { code: 'ENOENT' });
  });

  await t.test('an auxiliary attaches only to its selected TypeScript package', async () => {
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'two-packages'), [
      pluginTypeScript({ path: 'public' }),
      pluginTypeScript({ path: 'internal' }),
      pluginZod({ target: 'internal', output: 'validation' }),
    ]), { write: false });
    assert.ok(result.files.some((file) => file.path === 'internal/validation.ts'));
    assert.ok(!result.files.some((file) => file.path === 'public/validation.ts'));
  });

  await t.test('an auxiliary resolves equivalent package path spellings', async () => {
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'normalized-target'), [
      pluginTypeScript({ path: './web' }),
      pluginZod({ target: 'web', output: 'validation' }),
    ]), { write: false });
    assert.ok(result.files.some((file) => file.path === 'web/validation.ts'));
  });
});
