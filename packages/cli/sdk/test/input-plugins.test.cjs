'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const { availableInputPlugins, defineInputPlugin, generate, inspectInput } = require('../index.cjs');
const bundle = require('../../../node-plugins/all/index.cjs');
const { artifacts, config, root, temporary } = require('../test-support/helpers.cjs');

const cases = [
  ['graphql', bundle.inputGraphql, 'crates/inputs/graphql/tests/fixtures/github/schema.graphql'],
  ['asyncapi', bundle.inputAsyncApi, 'crates/inputs/asyncapi/tests/fixtures/events.yaml'],
  ['arazzo', bundle.inputArazzo, 'crates/inputs/arazzo/tests/fixtures/workflows.yaml'],
  ['protobuf', bundle.inputProtobuf, 'crates/inputs/protobuf/tests/fixtures/rpc/service.proto'],
  ['capnproto', bundle.inputCapnProto, 'crates/inputs/capnproto/tests/fixtures/rpc/service.capnp'],
];

test('Rust input providers are registered and inspect native contracts from Node', async () => {
  const registered = availableInputPlugins();
  assert.deepEqual(registered.map((item) => item.format).sort(),
    ['arazzo', 'asyncapi', 'capnproto', 'graphql', 'protobuf']);
  for (const [format, factory, relative] of cases) {
    if (format === 'capnproto' && spawnSync('capnp', ['--version']).error) {
      await assert.rejects(
        inspectInput({ path: path.join(root, relative), plugin: factory() }),
        /requires the official `capnp` compiler/,
      );
      continue;
    }
    const report = await inspectInput({ path: path.join(root, relative), plugin: factory() });
    assert.equal(report.summary.format, format);
    assert.equal(report.provider, factory().provider);
    assert.ok(report.summary.title);
    assert.ok(Array.isArray(report.summary.operations));
  }
});

test('Rust GraphQL input and JavaScript output plugin generate and regenerate together', async (t) => {
  const dir = await temporary(t);
  const output = path.join(dir, 'out');
  const input = { path: path.join(root, cases[0][2]), plugin: bundle.inputGraphql() };
  const configuration = { input, output, plugins: [{
    name: 'schema-report',
    requires: ['@relevate/kaji-input-graphql'],
    generate(ctx) {
      assert.equal(ctx.api, null);
      assert.equal(ctx.input.summary.format, 'graphql');
      ctx.emitFile({ path: 'graphql.json', contents: JSON.stringify(ctx.input.summary) });
    },
  }] };
  const first = await generate(configuration);
  assert.equal(first.api, null);
  assert.equal(first.input.provider, 'graphql.apollo');
  assert.ok(first.changes.added.includes('graphql.json'));
  assert.ok(JSON.parse(await fs.readFile(path.join(output, 'graphql.json'), 'utf8')).types.length);
  const second = await generate(configuration);
  assert.deepEqual(second.changes, { added: [], modified: [], removed: [] });
});

test('native input rejects HTTP SDK consumers without a matching contract', async (t) => {
  const dir = await temporary(t);
  await assert.rejects(generate({
    input: { path: path.join(root, cases[0][2]), plugin: bundle.inputGraphql() },
    output: path.join(dir, 'out'),
    plugins: [bundle.pluginTypeScript()],
  }), /did not publish kaji\.http-api/);
  await assert.rejects(fs.stat(path.join(dir, 'out')), { code: 'ENOENT' });
});

test('Node input plugin can publish data for a JavaScript output plugin', async (t) => {
  const dir = await temporary(t);
  const source = path.join(dir, 'notes.json');
  await fs.writeFile(source, JSON.stringify({ title: 'Notes', items: ['one', 'two'] }));
  const inputJson = defineInputPlugin(() => ({
    kind: 'js-input', name: 'example.notes', format: 'notes',
    async load(file) {
      const data = JSON.parse(await fs.readFile(file, 'utf8'));
      return { summary: { format: 'notes', title: data.title, version: null,
        types: [], operations: [] }, data };
    },
  }));
  const configuration = { input: { path: source, plugin: inputJson() }, output: path.join(dir, 'out'),
    plugins: [{ name: 'notes-output', generate(ctx) {
      ctx.emitFile({ path: 'notes.txt', contents: ctx.input.data.items.join('\n') });
    } }] };
  const result = await generate(configuration);
  assert.equal(result.input.summary.title, 'Notes');
  assert.equal(await fs.readFile(path.join(dir, 'out', 'notes.txt'), 'utf8'), 'one\ntwo');
  assert.equal((await inspectInput(configuration.input)).provider, 'example.notes');
});

test('Node input plugin can publish an HTTP API for a Rust SDK and JavaScript output', async (t) => {
  const dir = await temporary(t);
  const compiled = await artifacts(t);
  const baseline = await generate(config({ artifacts: compiled }, path.join(dir, 'baseline'), [
    { name: 'noop', generate() {} },
  ]), { write: false });
  const source = path.join(dir, 'api.json');
  await fs.writeFile(source, JSON.stringify({
    ...baseline.api,
    operations: baseline.api.operations.map((operation) => ({ ...operation, security: [] })),
  }));
  const inputApi = defineInputPlugin(() => ({
    kind: 'js-input', name: 'example.http-json', format: 'http-json',
    async load(file) {
      const api = JSON.parse(await fs.readFile(file, 'utf8'));
      return { summary: { format: 'http-json', title: api.name, version: api.version,
        types: api.schemas.map((schema) => schema.name), operations: [] }, api };
    },
  }));
  const result = await generate({ input: { path: source, plugin: inputApi() }, output: path.join(dir, 'out'),
    plugins: [bundle.pluginTypeScript(), bundle.pluginZod({ target: 'typescript', output: 'validation' }), { name: 'summary', generate(ctx) {
      assert.ok(ctx.readFile('typescript/package.json'));
      assert.match(ctx.readFile('typescript/validation.ts'), /WidgetSchema/);
      ctx.emitFile({ path: 'input.txt', contents: ctx.input.summary.title });
    } }] }, { write: false });
  assert.equal(result.api.name, baseline.api.name);
  assert.ok(result.files.some((file) => file.path === 'typescript/package.json'));
  assert.ok(result.files.some((file) => file.path === 'input.txt'));
});

test('invalid Node input output fails before generation', async (t) => {
  const dir = await temporary(t);
  const bad = defineInputPlugin(() => ({ kind: 'js-input', name: 'bad', format: 'bad',
    load() { return { summary: { format: 'wrong', title: 'Bad', types: [], operations: [] } }; } }));
  await assert.rejects(generate({ input: { path: path.join(dir, 'missing'), plugin: bad() },
    output: path.join(dir, 'out'), plugins: [{ name: 'unused', generate() {} }] }), /invalid summary/);
  await assert.rejects(fs.stat(path.join(dir, 'out')), { code: 'ENOENT' });
});

test('input provider errors stop JavaScript output before any file is written', async (t) => {
  const dir = await temporary(t);
  let generated = false;
  await assert.rejects(generate({
    input: { path: path.join(dir, 'missing.graphql'), plugin: bundle.inputGraphql() },
    output: path.join(dir, 'out'),
    plugins: [{ name: 'output', generate() { generated = true; } }],
  }), /failed reading|No such file|not found/i);
  assert.equal(generated, false);
  await assert.rejects(fs.stat(path.join(dir, 'out')), { code: 'ENOENT' });
});
