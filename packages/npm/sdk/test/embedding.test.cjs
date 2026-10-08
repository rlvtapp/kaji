'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { createPoolster, defineConfig, definePlugin } = require('../index.cjs');
const { pluginTypeScript } = require('../../plugins/typescript/index.cjs');
const fixture = path.resolve(__dirname, '../../../../examples/cli-basic/openapi.yaml');

async function temp(t) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'poolster-node-test-'));
  t.after(() => fs.rm(dir, { recursive: true, force: true }));
  return dir;
}

test('embeds Rust SDK generation and runs JavaScript plugin hooks', async (t) => {
  const dir = await temp(t);
  const output = path.join(dir, 'generated');
  const pluginNotes = definePlugin((suffix) => ({
    name: 'notes',
    transformApi(api) { api.name = 'Plugin Notes'; },
    generate(ctx) {
      const readme = ctx.readFile('typescript/README.md');
      assert.equal(typeof readme, 'string');
      ctx.replaceFile('typescript/README.md', `${readme}\n<!-- JS plugin -->\n`);
    },
    schema(schema, ctx) {
      ctx.emitFile({ path: `notes/${schema.name}.txt`, contents: schema.name });
    },
    operation(operation, ctx) {
      ctx.emitFile({ path: `notes/${operation.id}.txt`, contents: `${operation.method} ${operation.path}${suffix}\n` });
    },
  }));
  const config = defineConfig({
    input: fixture, output, name: 'Notes', version: '1.0.0',
    plugins: [pluginTypeScript({ path: 'typescript', name: '@acme/notes' }), pluginNotes('!')],
  });
  const result = await createPoolster(config).generate();
  assert.equal(result.api.name, 'Plugin Notes');
  assert.ok(result.files.some((file) => file.path.startsWith('typescript/')));
  assert.match(await fs.readFile(path.join(output, 'typescript/README.md'), 'utf8'), /JS plugin/);
  assert.equal(await fs.readFile(path.join(output, 'notes/Note.txt'), 'utf8'), 'Note');
  assert.equal(await fs.readFile(path.join(output, 'notes/getNote.txt'), 'utf8'), 'GET /notes/{noteId}!\n');
  assert.ok(result.changes.added.includes('notes/getNote.txt'));
  const again = await createPoolster(config).generate({ write: false });
  assert.deepEqual(again.changes, { added: [], modified: [], removed: [] });
});

test('plugin-only builds stay in memory and enforce safe paths', async (t) => {
  const dir = await temp(t);
  const output = path.join(dir, 'generated');
  const config = defineConfig({
    input: fixture, output, name: 'Notes', version: '1.0.0',
    plugins: [{ name: 'summary', generate(ctx) { ctx.emitFile({ path: 'summary.txt', contents: `${ctx.api.operations.length}\n` }); } }],
  });
  const result = await createPoolster(config).generate({ write: false });
  assert.equal(result.files.find((file) => file.path === 'summary.txt').contents, '1\n');
  await assert.rejects(fs.stat(output), { code: 'ENOENT' });
  const invalid = { ...config, plugins: [{ name: 'escape', generate(ctx) { ctx.emitFile({ path: '../escape.txt', contents: 'bad' }); } }] };
  await assert.rejects(createPoolster(invalid).generate(), /unsafe generated file path/);
});

test('plugin output honors ownership and rejects edits or duplicate paths', async (t) => {
  const dir = await temp(t);
  const output = path.join(dir, 'generated');
  const config = defineConfig({
    input: fixture, output, name: 'Notes', version: '1.0.0',
    plugins: [{ name: 'owner', generate(ctx) { ctx.emitFile({ path: 'mine.txt', contents: 'generated\n' }); } }],
  });
  await createPoolster(config).generate();
  await fs.writeFile(path.join(output, 'mine.txt'), 'hand edited\n');
  await assert.rejects(createPoolster(config).generate(), /edited|modified|overwrite/i);
  assert.equal(await fs.readFile(path.join(output, 'mine.txt'), 'utf8'), 'hand edited\n');
  const duplicate = { ...config, output: path.join(dir, 'other'), plugins: [
    { name: 'first', generate(ctx) { ctx.emitFile({ path: 'x.txt', contents: '1' }); } },
    { name: 'second', generate(ctx) { ctx.emitFile({ path: 'x.txt', contents: '2' }); } },
  ] };
  await assert.rejects(createPoolster(duplicate).generate(), /existing file/);
});
