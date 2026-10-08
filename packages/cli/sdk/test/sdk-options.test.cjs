'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const test = require('node:test');

const { generate } = require('../index.cjs');
const bundle = require('../../../node-plugins/all/index.cjs');
const { pluginTypeScript: individualTypeScript } = require('../../../node-plugins/typescript/index.cjs');
const { artifacts, config, temporary } = require('../test-support/helpers.cjs');

test('language plugin packages select native SDK renderers', async (t) => {
  const compiled = await artifacts(t);
  const dir = await temporary(t);

  await t.test('the bundle exposes all ten language factories', async () => {
    const factories = [
      ['typescript', bundle.pluginTypeScript],
      ['rust', bundle.pluginRust],
      ['go', bundle.pluginGo],
      ['python', bundle.pluginPython],
      ['php', bundle.pluginPhp],
      ['java', bundle.pluginJava],
      ['csharp', bundle.pluginCSharp],
      ['elixir', bundle.pluginElixir],
      ['ruby', bundle.pluginRuby],
      ['swift', bundle.pluginSwift],
    ];
    const plugins = factories.map(([language, factory]) => {
      const selected = factory();
      assert.equal(selected.package.language, language);
      assert.equal(selected.package.path, language);
      return selected;
    });
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'all'), plugins), { write: false });
    for (const [language] of factories) {
      assert.ok(result.files.some((file) => file.path.startsWith(`${language}/`)), `${language} did not emit files`);
    }
    assert.ok(result.files.length > 50);
  });

  await t.test('individual and bundle factories produce the same native selection', async () => {
    assert.deepEqual(individualTypeScript({ path: 'sdk', transport: 'axios' }), bundle.pluginTypeScript({ path: 'sdk', transport: 'axios' }));
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'two-ts'), [
      individualTypeScript({ path: 'fetch', transport: 'fetch', name: '@example/fetch' }),
      bundle.pluginTypeScript({ path: 'axios', transport: 'axios', name: '@example/axios' }),
    ]), { write: false });
    const fetchManifest = result.files.find((file) => file.path === 'fetch/package.json')?.contents;
    const axiosManifest = result.files.find((file) => file.path === 'axios/package.json')?.contents;
    assert.equal(typeof fetchManifest, 'string');
    assert.equal(typeof axiosManifest, 'string');
    assert.doesNotMatch(fetchManifest, /"axios"/);
    assert.match(axiosManifest, /"axios"/);
  });

  await t.test('package version and style options change native output', async () => {
    const regular = await generate(config({ artifacts: compiled }, path.join(dir, 'regular'), [
      bundle.pluginTypeScript({ path: 'typescript' }),
    ]), { write: false });
    const changed = await generate(config({ artifacts: compiled }, path.join(dir, 'changed'), [
      bundle.pluginTypeScript({ path: 'typescript', version: '2.5.0', style: 'flat', raw: true, clientName: 'WidgetsClient' }),
    ]), { write: false });
    const manifest = changed.files.find((file) => file.path === 'typescript/package.json')?.contents;
    assert.equal(JSON.parse(manifest).version, '2.5.0');
    assert.notDeepEqual(regular.files, changed.files);
  });

  await t.test('unsupported combinations reject before writing', async () => {
    const cases = [
      [bundle.pluginTypeScript({ transport: 'bogus' }), /transport must be fetch or axios/],
      [bundle.pluginTypeScript({ style: 'diagonal' }), /style must be flat or namespaced/],
      [bundle.pluginGo({ jobs: 0 }), /jobs must be at least 1/],
      [bundle.pluginGo({ transport: 'axios' }), /TypeScript-only/],
      [bundle.pluginPython({ raw: true }), /TypeScript-only/],
      [bundle.pluginTypeScript({ unknown: true }), /unknown sdk option unknown/],
    ];
    for (const [index, [plugin, expected]] of cases.entries()) {
      await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, `invalid-${index}`), [plugin])), expected);
    }
  });

  await t.test('duplicate SDK package paths fail before loading input or writing output', async () => {
    const output = path.join(dir, 'duplicate-package-path');
    for (const duplicate of ['shared', './shared', 'shared/', 'SHARED']) {
      await assert.rejects(generate(config(path.join(dir, 'missing-spec.yaml'), output, [
        bundle.pluginTypeScript({ path: 'shared' }),
        bundle.pluginRust({ path: duplicate }),
      ])), /SDK package path .* is configured twice/);
    }
    await assert.rejects(fs.stat(output), { code: 'ENOENT' });
  });
});
