'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const test = require('node:test');

const { defineConfig, generate } = require('../index.cjs');
const { pluginTypeScript } = require('../../../node-plugins/typescript/index.cjs');
const { artifacts, config, fixture, temporary } = require('../test-support/helpers.cjs');

test('source and precompiled artifact inputs render identical SDK files', async (t) => {
  const dir = await temporary(t);
  const compiled = await artifacts(t);
  const plugins = [pluginTypeScript({ path: 'typescript', name: '@example/widgets' })];
  const fromSource = await generate(config(fixture, path.join(dir, 'source'), plugins), { write: false });
  const fromArtifacts = await generate(config({ artifacts: compiled }, path.join(dir, 'artifacts'), plugins), { write: false });
  assert.deepEqual(fromSource.api, fromArtifacts.api);
  assert.deepEqual(fromSource.files, fromArtifacts.files);
  assert.ok(fromArtifacts.files.length > 5);
  await assert.rejects(fs.stat(path.join(dir, 'source')), { code: 'ENOENT' });
  await assert.rejects(fs.stat(path.join(dir, 'artifacts')), { code: 'ENOENT' });
});

test('plugins receive normalized operations, schemas, and named security catalog', async (t) => {
  const dir = await temporary(t);
  const compiled = await artifacts(t);
  const seen = {};
  const result = await generate(config({ artifacts: compiled }, path.join(dir, 'out'), [{
    name: 'inspect',
    generate(ctx) {
      seen.operations = ctx.api.operations.map((operation) => operation.id);
      seen.schemas = ctx.api.schemas.map((schema) => schema.name);
      seen.security = ctx.securitySchemes;
    },
  }]), { write: false });
  assert.deepEqual(result.files, []);
  assert.deepEqual(seen.operations.sort(), ['createWidget', 'getWidget', 'internalHealth', 'listWidgets']);
  assert.deepEqual(seen.schemas.sort(), ['CreateWidget', 'Widget']);
  assert.match(JSON.stringify(seen.security), /WidgetKey/);
});

test('artifact input skips the compiler even when its override is invalid', async (t) => {
  const dir = await temporary(t);
  const compiled = await artifacts(t);
  const plugin = { name: 'count', generate(ctx) { ctx.emitFile({ path: 'count.txt', contents: String(ctx.api.operations.length) }); } };
  const result = await generate(config({ artifacts: compiled }, path.join(dir, 'out'), [plugin], {
    compiler: path.join(dir, 'missing-compiler'),
  }), { write: false });
  assert.equal(result.files[0].contents, '4');
});

test('missing source and malformed specs fail without writing output', async (t) => {
  const dir = await temporary(t);
  const output = path.join(dir, 'out');
  const plugin = { name: 'marker', generate(ctx) { ctx.emitFile({ path: 'marker.txt', contents: 'ok' }); } };
  await assert.rejects(generate(config(path.join(dir, 'missing.yaml'), output, [plugin])), /compiler failed|no such file|cannot/i);
  const malformed = path.join(dir, 'malformed.yaml');
  await fs.writeFile(malformed, 'openapi: [not a valid document\n');
  await assert.rejects(generate(config(malformed, output, [plugin])), /compiler failed/i);
  await assert.rejects(fs.stat(output), { code: 'ENOENT' });
});

test('missing artifact files reject before any plugin runs', async (t) => {
  const dir = await temporary(t);
  let called = false;
  await assert.rejects(generate(defineConfig(config({ artifacts: dir }, path.join(dir, 'out'), [{
    name: 'not-run', generate() { called = true; },
  }]))), /operations\.json|No such file|not found/i);
  assert.equal(called, false);
});
