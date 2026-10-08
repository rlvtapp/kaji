'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const test = require('node:test');

const { definePlugin, generate } = require('../index.cjs');
const { pluginTypeScript } = require('../../plugins/typescript/index.cjs');
const { artifacts, config, temporary } = require('../test-support/helpers.cjs');

test('JavaScript plugins compose around native SDK renderers', async (t) => {
  const compiled = await artifacts(t);
  const dir = await temporary(t);

  await t.test('dependencies sort transform and generation hooks before dependents', async () => {
    const events = [];
    const first = definePlugin(() => ({
      name: 'first',
      requires: ['second', '@relevate/poolster-plugin-typescript'],
      hooks: {
        transformApi(api) {
          events.push(`first:transform:${api.name}`);
          return { ...api, name: 'First' };
        },
        async generate(ctx) {
          await Promise.resolve();
          events.push(`first:generate:${ctx.api.name}`);
          assert.match(ctx.readFile('typescript/README.md'), /Widgets|First/);
          assert.equal(ctx.readFile('second.txt'), 'First');
          ctx.emitFile({ path: 'first.txt', contents: ctx.api.name });
        },
        schema(schema) { events.push(`first:schema:${schema.name}`); },
        operation(operation) { events.push(`first:operation:${operation.id}`); },
      },
    }));
    const second = definePlugin(() => ({
      name: 'second',
      hooks: {
        transformApi(api) { events.push('second:transform'); api.name = 'Second'; },
        generate(ctx) { events.push('second:generate'); ctx.emitFile({ path: 'second.txt', contents: ctx.api.name }); },
      },
    }));
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'ordered'), [
      first(), pluginTypeScript(), second(),
    ]), { write: false });
    assert.deepEqual(events.slice(0, 4), ['second:transform', 'first:transform:Second', 'second:generate', 'first:generate:First']);
    assert.equal(events.filter((event) => event.startsWith('first:schema:')).length, 2);
    assert.equal(events.filter((event) => event.startsWith('first:operation:')).length, 4);
    assert.equal(result.files.find((file) => file.path === 'first.txt').contents, 'First');
  });

  await t.test('missing dependencies and cycles fail before compiler or output work', async () => {
    const missing = { name: 'dependent', requires: ['absent'], hooks: { generate() {} } };
    await assert.rejects(generate(config(path.join(dir, 'missing-spec.yaml'), path.join(dir, 'missing'), [missing])), /requires missing plugin absent/);
    const a = { name: 'a', requires: ['b'], hooks: { generate() {} } };
    const b = { name: 'b', requires: ['a'], hooks: { generate() {} } };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'cycle'), [a, b])), /dependency cycle/);
    const collision = { name: '@relevate/poolster-plugin-typescript', hooks: { generate() {} } };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'collision-a'), [collision, pluginTypeScript()])), /duplicate plugin name/);
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'collision-b'), [pluginTypeScript(), collision])), /duplicate plugin name/);
  });

  await t.test('hook failure leaves output untouched', async () => {
    const output = path.join(dir, 'failure');
    const plugin = { name: 'broken', hooks: { generate(ctx) { ctx.emitFile({ path: 'partial.txt', contents: 'partial' }); throw new Error('plugin failed'); } } };
    await assert.rejects(generate(config({ artifacts: compiled }, output, [plugin])), /plugin failed/);
    await assert.rejects(fs.stat(output), { code: 'ENOENT' });
  });

  await t.test('a transform can remove internal operations before native rendering', async () => {
    const filter = { name: 'public-only', hooks: { transformApi(api) {
      return { ...api, operations: api.operations.filter((operation) => !operation.path.startsWith('/internal/')) };
    } } };
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'filtered'), [filter, pluginTypeScript()]), { write: false });
    assert.equal(result.api.operations.length, 3);
    assert.ok(result.files.every((file) => !file.path.toLowerCase().includes('internalhealth')));
  });

  await t.test('create-once files preserve author edits while stale owned files are removed', async () => {
    const output = path.join(dir, 'ownership');
    const first = { name: 'custom', hooks: { generate(ctx) {
      ctx.emitFile({ path: 'custom.txt', contents: 'starter\n', preserveExisting: true });
      ctx.emitFile({ path: 'stale.txt', contents: 'obsolete\n' });
    } } };
    await generate(config({ artifacts: compiled }, output, [first]));
    await fs.writeFile(path.join(output, 'custom.txt'), 'author edit\n');
    await fs.writeFile(path.join(output, 'unrelated.txt'), 'keep me\n');
    const next = { name: 'custom', hooks: { generate(ctx) {
      ctx.emitFile({ path: 'custom.txt', contents: 'new starter\n', preserveExisting: true });
    } } };
    const result = await generate(config({ artifacts: compiled }, output, [next]));
    assert.ok(result.changes.removed.includes('stale.txt'));
    assert.equal(await fs.readFile(path.join(output, 'custom.txt'), 'utf8'), 'author edit\n');
    assert.equal(await fs.readFile(path.join(output, 'unrelated.txt'), 'utf8'), 'keep me\n');
    await assert.rejects(fs.stat(path.join(output, 'stale.txt')), { code: 'ENOENT' });
  });

  await t.test('dry-run reports edited owned files without modifying them', async () => {
    const output = path.join(dir, 'drift');
    const plugin = { name: 'report', hooks: { generate(ctx) { ctx.emitFile({ path: 'report.txt', contents: 'generated\n' }); } } };
    await generate(config({ artifacts: compiled }, output, [plugin]));
    await fs.writeFile(path.join(output, 'report.txt'), 'edited\n');
    const preview = await generate(config({ artifacts: compiled }, output, [plugin]), { write: false });
    assert.deepEqual(preview.changes.modified, ['report.txt']);
    assert.equal(await fs.readFile(path.join(output, 'report.txt'), 'utf8'), 'edited\n');
    await assert.rejects(generate(config({ artifacts: compiled }, output, [plugin])), /edited|modified|overwrite/i);
  });

  await t.test('native collisions and reserved paths are rejected', async () => {
    const collide = { name: 'collide', hooks: { generate(ctx) { ctx.emitFile({ path: 'typescript/README.md', contents: 'replace' }); } } };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'collision'), [pluginTypeScript(), collide])), /existing file/);
    const caseCollision = { name: 'case-collision', hooks: { generate(ctx) {
      ctx.emitFile({ path: 'TypeScript/README.md', contents: 'replace' });
    } } };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'case-collision'), [
      pluginTypeScript(), caseCollision,
    ])), /existing file/);
    const jsCaseCollision = { name: 'js-case-collision', hooks: { generate(ctx) {
      ctx.emitFile({ path: 'Guide.txt', contents: 'first' });
      ctx.emitFile({ path: 'guide.txt', contents: 'second' });
    } } };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'js-case-collision'), [
      jsCaseCollision,
    ])), /existing file/);
    const reserved = { name: 'reserved', hooks: { generate(ctx) { ctx.emitFile({ path: '.poolster/ownership.json', contents: '{}' }); } } };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'reserved'), [reserved])), /reserved Poolster ownership path/);
  });
});
