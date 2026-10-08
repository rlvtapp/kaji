'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { pathToFileURL } = require('node:url');

const root = path.resolve(__dirname, '../../..');
const core = require('../package.json');
const bundle = require('../../plugins-all/index.cjs');
const languages = [
  ['typescript', 'TypeScript'], ['rust', 'Rust'], ['go', 'Go'],
  ['python', 'Python'], ['php', 'Php'], ['java', 'Java'],
  ['csharp', 'CSharp'], ['elixir', 'Elixir'], ['ruby', 'Ruby'], ['swift', 'Swift'],
];
const auxiliaries = [
  ['zod', 'Zod'], ['faker', 'Faker'], ['msw', 'Msw'],
  ['cypress', 'Cypress'], ['react-query', 'ReactQuery'],
  ['vue-query', 'VueQuery'], ['swr', 'Swr'],
];

test('SDK package installs only SDK platform runtimes', () => {
  assert.equal(core.name, '@relevate/poolster');
  assert.deepEqual(Object.keys(core.optionalDependencies).sort(), [
    '@relevate/poolster-node-darwin-arm64', '@relevate/poolster-node-darwin-x64',
    '@relevate/poolster-node-linux-x64-gnu', '@relevate/poolster-node-win32-x64-msvc',
  ].sort());
  assert.equal(core.dependencies?.poolster, undefined);
  assert.equal(core.bin, undefined);
});

test('the published SDK and plugin subpaths resolve from @relevate/poolster', async () => {
  const sdk = require('@relevate/poolster/sdk');
  const plugins = require('@relevate/poolster/sdk/plugins');
  const sdkEsm = await import('@relevate/poolster/sdk');
  const pluginsEsm = await import('@relevate/poolster/sdk/plugins');
  assert.equal(typeof sdk.defineConfig, 'function');
  assert.equal(sdkEsm.defineConfig, sdk.defineConfig);
  assert.deepEqual(plugins.inputGraphql(), bundle.inputGraphql());
  assert.deepEqual(pluginsEsm.pluginTypeScript(), bundle.pluginTypeScript());
});
const inputs = [
  ['graphql', 'Graphql', 'graphql.apollo'],
  ['asyncapi', 'AsyncApi', 'asyncapi.roas'],
  ['arazzo', 'Arazzo', 'arazzo.roas'],
  ['protobuf', 'Protobuf', 'protobuf.protox'],
  ['capnproto', 'CapnProto', 'capnproto.capnp'],
];

test('native input providers have individual and bundled JS exports', async () => {
  for (const [format, suffix, provider] of inputs) {
    const directory = path.join(root, 'npm', 'inputs', format);
    const manifest = require(path.join(directory, 'package.json'));
    const factory = `input${suffix}`;
    assert.equal(manifest.name, `@relevate/poolster-input-${format}`);
    assert.equal(manifest.version, core.version);
    const cjs = require(path.join(directory, manifest.exports['.'].require));
    const esm = await import(pathToFileURL(path.join(directory, manifest.exports['.'].import)).href);
    assert.deepEqual(cjs[factory](), esm[factory]());
    assert.deepEqual(cjs[factory](), bundle[factory]());
    assert.equal(cjs[factory]().provider, provider);
  }
});

test('each language package has independently importable CommonJS and ESM exports', async () => {
  for (const [language, suffix] of languages) {
    const directory = path.join(root, 'npm', 'plugins', language);
    const manifest = require(path.join(directory, 'package.json'));
    const name = `@relevate/poolster-plugin-${language}`;
    const exportName = `plugin${suffix}`;
    assert.equal(manifest.name, name);
    assert.equal(manifest.version, core.version);
    assert.equal(manifest.peerDependencies['@relevate/poolster'], core.version);
    for (const file of ['index.cjs', 'index.mjs', 'index.d.ts', 'README.md', 'LICENSE']) {
      assert.ok(fs.statSync(path.join(directory, file)).size > 0, `${name} lacks ${file}`);
    }
    const cjs = require(path.join(directory, manifest.exports['.'].require));
    const esm = await import(pathToFileURL(path.join(directory, manifest.exports['.'].import)).href);
    const selected = cjs[exportName]({ path: 'client' });
    assert.deepEqual(esm[exportName]({ path: 'client' }), selected);
    assert.deepEqual(bundle[exportName]({ path: 'client' }), selected);
    assert.equal(selected.package.language, language);
    assert.equal(selected.name, name);
  }
});

test('each Rust auxiliary has independent CommonJS and ESM exports', async () => {
  for (const [id, suffix] of auxiliaries) {
    const directory = path.join(root, 'npm', 'plugins', id);
    const manifest = require(path.join(directory, 'package.json'));
    const exportName = `plugin${suffix}`;
    assert.equal(manifest.name, `@relevate/poolster-plugin-${id}`);
    assert.equal(manifest.peerDependencies['@relevate/poolster'], core.version);
    const cjs = require(path.join(directory, manifest.exports['.'].require));
    const esm = await import(pathToFileURL(path.join(directory, manifest.exports['.'].import)).href);
    assert.deepEqual(cjs[exportName]({ target: 'web' }), esm[exportName]({ target: 'web' }));
    assert.deepEqual(cjs[exportName]({ target: 'web' }), bundle[exportName]({ target: 'web' }));
  }
});

test('bundle package exposes the same named factories without pulling in individual packages', () => {
  const manifest = require('../../plugins-all/package.json');
  assert.equal(manifest.name, '@relevate/poolster-plugins');
  assert.deepEqual(Object.keys(bundle).sort(), [
    ...[...languages, ...auxiliaries].map(([, suffix]) => `plugin${suffix}`),
    ...inputs.map(([, suffix]) => `input${suffix}`),
  ].sort());
  assert.deepEqual(manifest.peerDependencies, { '@relevate/poolster': core.version });
  assert.equal(manifest.dependencies, undefined);
});
