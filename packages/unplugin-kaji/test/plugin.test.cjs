'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const Module = require('node:module');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const kaji = require('../src/index.cjs');

const adapterFiles = ['unplugin', 'vite', 'rollup', 'webpack', 'esbuild', 'rspack', 'rolldown', 'farm', 'astro', 'nuxt'];

function clearAdapterModules() {
  for (const name of adapterFiles) {
    delete require.cache[require.resolve(`../src/${name}.cjs`)];
  }
}

function withMocks(mocks, callback) {
  const load = Module._load;
  clearAdapterModules();
  Module._load = function mockedLoad(request, parent, isMain) {
    if (Object.hasOwn(mocks, request)) return mocks[request];
    return load.call(this, request, parent, isMain);
  };
  try {
    return callback();
  } finally {
    Module._load = load;
    clearAdapterModules();
  }
}

function unpluginMock() {
  return {
    createUnplugin(factory) {
      return Object.fromEntries(
        ['vite', 'rollup', 'webpack', 'esbuild', 'rspack', 'rolldown', 'farm'].map((adapter) => [
          adapter,
          (options) => ({ adapter, plugin: factory(options) }),
        ]),
      );
    },
  };
}

function fixture() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'unplugin-kaji-'));
  return {
    directory,
    remove() { fs.rmSync(directory, { recursive: true, force: true }); },
  };
}

test('runs explicit Kaji arguments before the bundler starts', async (t) => {
  const sandbox = fixture();
  t.after(() => sandbox.remove());
  const output = [];
  const plugin = kaji({
    config: false,
    command: process.execPath,
    args: ['-e', 'console.log("generated")'],
    cwd: sandbox.directory,
    silent: true,
    onGenerate: (result) => output.push(result.stdout),
  });

  const result = await plugin.buildStart.call({ addWatchFile() {} });
  assert.equal(result.code, 0);
  assert.deepEqual(output, ['generated\n']);
});

test('watches the recipe, local OpenAPI input, and explicit local dependencies', async (t) => {
  const sandbox = fixture();
  t.after(() => sandbox.remove());
  fs.writeFileSync(path.join(sandbox.directory, 'kaji.json'), JSON.stringify({
    openapi: { input: './openapi.yaml' },
  }));
  fs.writeFileSync(path.join(sandbox.directory, 'openapi.yaml'), 'openapi: 3.1.0');
  fs.writeFileSync(path.join(sandbox.directory, 'shared.yaml'), 'components: {}');
  const watched = [];
  const plugin = kaji({
    command: process.execPath,
    args: ['-e', ''],
    cwd: sandbox.directory,
    watchFiles: ['shared.yaml'],
    silent: true,
  });
  await plugin.buildStart.call({ addWatchFile: (file) => watched.push(file) });

  assert.deepEqual(new Set(watched), new Set([
    path.join(sandbox.directory, 'kaji.json'),
    path.join(sandbox.directory, 'openapi.yaml'),
    path.join(sandbox.directory, 'shared.yaml'),
  ]));
});

test('regenerates only for registered files in watch mode', async (t) => {
  const sandbox = fixture();
  t.after(() => sandbox.remove());
  const config = path.join(sandbox.directory, 'kaji.json');
  fs.writeFileSync(config, '{}');
  let runs = 0;
  const plugin = kaji({
    command: process.execPath,
    args: ['-e', ''],
    cwd: sandbox.directory,
    silent: true,
    onGenerate: () => { runs += 1; },
  });
  const context = { addWatchFile() {} };
  await plugin.buildStart.call(context);
  assert.equal(await plugin.watchChange.call(context, path.join(sandbox.directory, 'unrelated.ts')), undefined);
  await plugin.watchChange.call(context, config);
  assert.equal(runs, 2);
});

test('keeps subsequent watch runs available after a generator error', async (t) => {
  const sandbox = fixture();
  t.after(() => sandbox.remove());
  const config = path.join(sandbox.directory, 'kaji.json');
  fs.writeFileSync(config, '{}');
  const plugin = kaji({
    command: process.execPath,
    args: ['-e', 'process.exit(7)'],
    cwd: sandbox.directory,
    silent: true,
  });
  const context = { addWatchFile() {} };
  await assert.rejects(plugin.buildStart.call(context), /Kaji generation failed/);
  await assert.rejects(plugin.watchChange.call(context, config), /Kaji generation failed/);
});

test('exposes native unplugin adapters for Rspack, Rolldown, and Farm', () => {
  withMocks({ unplugin: unpluginMock() }, () => {
    for (const adapter of ['rspack', 'rolldown', 'farm']) {
      const createPlugin = require(`../src/${adapter}.cjs`);
      const result = createPlugin({ config: false, args: ['generate'] });
      assert.equal(result.adapter, adapter);
      assert.equal(result.plugin.name, 'kaji');
    }
  });
});

test('Astro integration appends Kaji to Astro Vite plugins', () => {
  withMocks({ unplugin: unpluginMock() }, () => {
    const astroKaji = require('../src/astro.cjs');
    const integration = astroKaji({ config: false, args: ['generate'] });
    const astro = { config: {} };
    integration.hooks['astro:config:setup'](astro);

    assert.equal(integration.name, '@relevate/unplugin-kaji');
    assert.equal(astro.config.vite.plugins.length, 1);
    assert.equal(astro.config.vite.plugins[0].adapter, 'vite');
  });
});

test('Nuxt module registers both supported builders', () => {
  const added = [];
  withMocks({
    unplugin: unpluginMock(),
    '@nuxt/kit': {
      defineNuxtModule: (definition) => definition,
      addVitePlugin: (plugin) => added.push(['vite', plugin]),
      addWebpackPlugin: (plugin) => added.push(['webpack', plugin]),
    },
  }, () => {
    const nuxtKaji = require('../src/nuxt.cjs');
    nuxtKaji.setup({ config: false, args: ['generate'] });

    assert.equal(nuxtKaji.meta.configKey, 'kaji');
    assert.deepEqual(added.map(([builder]) => builder), ['vite', 'webpack']);
    assert.equal(added[0][1]().adapter, 'vite');
    assert.equal(added[1][1]().adapter, 'webpack');
  });
});
