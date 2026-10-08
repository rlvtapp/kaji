'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const test = require('node:test');

const { artifacts, compiler, execFileAsync, root, temporary } = require('../test-support/helpers.cjs');

async function run(script, output, args = []) {
  return execFileAsync(process.execPath, [path.join(root, 'examples/node-embedded', script), ...args], {
    cwd: root,
    env: { ...process.env, KAJI_OPENAPI_BIN: compiler, KAJI_EXAMPLE_OUTPUT: output },
  });
}

test('checked-in Node embedding examples execute against the real compiler and addon', async (t) => {
  const dir = await temporary(t);

  await t.test('basic example emits a TypeScript SDK and operation catalog', async () => {
    const output = path.join(dir, 'basic');
    const { stdout } = await run('basic.mjs', output);
    assert.match(stdout, /Wrote \d+ files/);
    assert.match(await fs.readFile(path.join(output, 'catalog/README.md'), 'utf8'), /4 operations, 2 schemas/);
    assert.match(await fs.readFile(path.join(output, 'catalog/createWidget.md'), 'utf8'), /POST \/widgets/);
    assert.ok((await fs.readdir(path.join(output, 'typescript'))).length > 0);
  });

  await t.test('transform example filters the native Python SDK and catalog together', async () => {
    const output = path.join(dir, 'filtered');
    await run('transform.mjs', output);
    assert.match(await fs.readFile(path.join(output, 'public-api/README.md'), 'utf8'), /3 operations/);
    await assert.rejects(fs.stat(path.join(output, 'public-api/internalHealth.md')), { code: 'ENOENT' });
    assert.ok((await fs.readdir(path.join(output, 'python'))).length > 0);
  });

  await t.test('artifact example previews a Rust SDK without writing', async () => {
    const compiled = await artifacts(t);
    const output = path.join(dir, 'artifact-preview');
    const { stdout } = await run('artifacts.mjs', output, [compiled]);
    assert.match(stdout, /Preview: \d+ files; \d+ would be added/);
    await assert.rejects(fs.stat(output), { code: 'ENOENT' });
  });

  await t.test('native plugin example combines Rust output with JS contract consumers', async () => {
    const output = path.join(dir, 'native-contracts');
    const { stdout } = await run('native-contracts.mjs', output);
    assert.match(stdout, /Rust plugin output and a JS post-plugin report/);
    assert.deepEqual(JSON.parse(await fs.readFile(path.join(output, 'native-report.json'), 'utf8')), {
      validation: true,
      queries: true,
    });
    assert.ok((await fs.readFile(path.join(output, 'web/validation.ts'), 'utf8')).includes('zod'));
  });

  await t.test('input examples combine Rust and Node parsers with JS outputs', async () => {
    const output = path.join(dir, 'inputs');
    const { stdout } = await run('input-plugins.mjs', output);
    assert.match(stdout, /Rust GraphQL input exposed \d+ types/);
    assert.match(stdout, /Example catalog/);
    await assert.rejects(fs.stat(output), { code: 'ENOENT' });
  });
});
