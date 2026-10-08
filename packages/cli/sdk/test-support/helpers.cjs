'use strict';

const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { execFile } = require('node:child_process');
const { promisify } = require('node:util');

const execFileAsync = promisify(execFile);
const root = path.resolve(__dirname, '../../../..');
const fixture = path.join(root, 'examples', 'node-embedded', 'openapi.yaml');
const compiler = process.env.KAJI_OPENAPI_BIN || path.join(root, 'target', 'debug', process.platform === 'win32' ? 'kaji-openapi.exe' : 'kaji-openapi');

async function temporary(t) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'kaji-node-test-'));
  t.after(() => fs.rm(dir, { recursive: true, force: true }));
  return dir;
}

async function artifacts(t, source = fixture) {
  const dir = await temporary(t);
  await execFileAsync(compiler, ['--out', dir, source], { maxBuffer: 1024 * 1024 });
  return dir;
}

function config(input, output, plugins, extra = {}) {
  return { input, output, name: 'Widgets', version: '1.0.0', plugins, ...extra };
}

module.exports = { root, fixture, compiler, temporary, artifacts, config, execFileAsync };
