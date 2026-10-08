'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { pathToFileURL } = require('node:url');
const test = require('node:test');

const { generate, loadConfig } = require('../index.cjs');
const { compiler, fixture, root, temporary } = require('../test-support/helpers.cjs');

const cli = path.join(root, 'packages/npm/cli/bin/poolster.cjs');
const core = path.join(root, 'packages/npm/sdk/index.cjs');
const tsPlugin = path.join(root, 'packages/npm/plugins/typescript/index.cjs');

function runCli(args, cwd) {
  return spawnSync(process.execPath, [cli, ...args], {
    cwd,
    encoding: 'utf8',
    env: { ...process.env, POOLSTER_OPENAPI_BIN: compiler },
  });
}

test('JavaScript config files drive the Node CLI', async (t) => {
  const dir = await temporary(t);
  await fs.copyFile(fixture, path.join(dir, 'api.yaml'));
  await fs.mkdir(path.join(dir, 'node_modules/@relevate'), { recursive: true });
  await fs.symlink(path.join(root, 'packages/npm/sdk'), path.join(dir, 'node_modules/@relevate/poolster'), 'dir');

  await t.test('auto-discovered CommonJS config resolves paths from its own directory', async () => {
    const source = `const { defineConfig } = require(${JSON.stringify(core)});\nconst { pluginTypeScript } = require(${JSON.stringify(tsPlugin)});\nmodule.exports = defineConfig({ input: './api.yaml', output: { path: './generated' }, name: 'Widgets', version: '1.0.0', plugins: [pluginTypeScript(), { name: 'status', hooks: { generate(ctx) { ctx.emitFile({ path: 'status.txt', contents: 'ready\\n' }); } } }] });\n`;
    await fs.writeFile(path.join(dir, 'poolster.config.cjs'), source);
    const first = runCli(['generate'], dir);
    assert.equal(first.status, 0, first.stderr);
    assert.match(first.stdout, /added/);
    assert.equal(await fs.readFile(path.join(dir, 'generated/status.txt'), 'utf8'), 'ready\n');
    assert.ok((await fs.readdir(path.join(dir, 'generated/typescript'))).length > 0);
    const clean = runCli(['generate', '--check'], dir);
    assert.equal(clean.status, 0, clean.stderr);
    assert.match(clean.stdout, /0 added, 0 modified, 0 removed/);
    await fs.writeFile(path.join(dir, 'generated/status.txt'), 'author edit\n');
    const drift = runCli(['generate', '--check'], dir);
    assert.equal(drift.status, 1);
    assert.match(drift.stdout, /1 modified/);
    assert.equal(await fs.readFile(path.join(dir, 'generated/status.txt'), 'utf8'), 'author edit\n');
  });

  await t.test('explicit ESM config can export an async factory', async () => {
    const file = path.join(dir, 'other.config.mjs');
    const source = `import { defineConfig } from ${JSON.stringify(pathToFileURL(core).href)};\nexport default async () => defineConfig({ input: './api.yaml', output: './esm-output', name: 'Widgets', version: '1.0.0', plugins: [{ name: 'esm', hooks: { generate(ctx) { ctx.emitFile({ path: 'esm.txt', contents: 'esm' }); } } }] });\n`;
    await fs.writeFile(file, source);
    const loaded = await loadConfig(file);
    assert.equal(loaded.input, path.join(dir, 'api.yaml'));
    assert.equal(loaded.output, path.join(dir, 'esm-output'));
    const preview = await generate(loaded, { write: false });
    assert.equal(preview.files[0].contents, 'esm');
    const cliPreview = runCli(['generate', '--config', file, '--dry-run'], root);
    assert.equal(cliPreview.status, 0, cliPreview.stderr);
    await assert.rejects(fs.stat(path.join(dir, 'esm-output')), { code: 'ENOENT' });
  });

  await t.test('ESM config mixes registered Rust plugins and a JavaScript post plugin', async () => {
    const mixed = path.join(dir, 'mixed.config.mjs');
    const ts = path.join(root, 'packages/npm/plugins/typescript/index.mjs');
    const zod = path.join(root, 'packages/npm/plugins/zod/index.mjs');
    const react = path.join(root, 'packages/npm/plugins/react-query/index.mjs');
    const source = `import { defineConfig } from ${JSON.stringify(pathToFileURL(path.join(root, 'packages/npm/sdk/index.mjs')).href)};
import { pluginTypeScript } from ${JSON.stringify(pathToFileURL(ts).href)};
import { pluginZod } from ${JSON.stringify(pathToFileURL(zod).href)};
import { pluginReactQuery } from ${JSON.stringify(pathToFileURL(react).href)};
export default defineConfig({ input: './api.yaml', output: './mixed-output', name: 'Widgets', version: '1.0.0', plugins: [pluginTypeScript({ path: 'web' }), pluginZod({ target: 'web', output: 'validation' }), pluginReactQuery({ target: 'web', output: 'queries' }), { name: 'audit', phase: 'post', generate(ctx) { ctx.emitFile({ path: 'audit.txt', contents: String(ctx.readFile('web/validation.ts')?.includes('WidgetSchema') && ctx.readFile('web/queries.ts')?.includes('listWidgetsQueryKey')) }); } }] });
`;
    await fs.writeFile(mixed, source);
    const first = runCli(['generate', '--config', mixed], root);
    assert.equal(first.status, 0, first.stderr);
    assert.equal(await fs.readFile(path.join(dir, 'mixed-output/audit.txt'), 'utf8'), 'true');
    assert.match(await fs.readFile(path.join(dir, 'mixed-output/web/validation.ts'), 'utf8'), /WidgetSchema/);
    const check = runCli(['generate', '--config', mixed, '--check'], root);
    assert.equal(check.status, 0, check.stderr);
    assert.match(check.stdout, /0 added, 0 modified, 0 removed/);
  });

  await t.test('missing config and invalid flags report actionable errors', () => {
    const noConfig = runCli(['generate', '--config', path.join(dir, 'absent.mjs')], root);
    assert.equal(noConfig.status, 1);
    assert.match(noConfig.stderr, /No Poolster config found/);
    const badFlag = runCli(['generate', '--unknown'], dir);
    assert.equal(badFlag.status, 1);
    assert.match(badFlag.stderr, /Unknown option/);
  });
});

test('ESM entry points expose the config and language plugin factories', async () => {
  const coreModule = await import(pathToFileURL(path.join(root, 'packages/npm/sdk/index.mjs')).href);
  const languageModule = await import(pathToFileURL(path.join(root, 'packages/npm/plugins/typescript/index.mjs')).href);
  const bundleModule = await import(pathToFileURL(path.join(root, 'packages/npm/plugins-all/index.mjs')).href);
  assert.equal(typeof coreModule.loadConfig, 'function');
  assert.equal(typeof coreModule.definePlugin, 'function');
  assert.equal(typeof coreModule.defineContract, 'function');
  assert.equal(typeof coreModule.providerHandle, 'function');
  assert.equal(typeof coreModule.requireContract, 'function');
  assert.ok(coreModule.availableNativePlugins().includes('typescript/zod'));
  assert.deepEqual(languageModule.pluginTypeScript(), bundleModule.pluginTypeScript());
});
