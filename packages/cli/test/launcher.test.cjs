'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const path = require('node:path');
const { platformPackage } = require('../bin/poolster.cjs');

test('CLI package installs only CLI platform runtimes', () => {
  const manifest = require('../package.json');
  assert.equal(manifest.name, 'poolster');
  assert.deepEqual(Object.keys(manifest.optionalDependencies).sort(), [
    '@relevate/poolster-cli-darwin-arm64', '@relevate/poolster-cli-darwin-x64',
    '@relevate/poolster-cli-linux-x64-gnu', '@relevate/poolster-cli-win32-x64-msvc',
  ].sort());
  assert.equal(manifest.dependencies?.['@relevate/poolster'], undefined);
  assert.deepEqual(Object.keys(manifest.bin), ['poolster']);
  assert.ok(!manifest.files.some(file => file.startsWith('sdk/')));
});

test('resolves supported platforms without installing or downloading anything', () => {
  assert.equal(platformPackage('darwin', 'arm64'), '@relevate/poolster-cli-darwin-arm64');
  assert.equal(platformPackage('darwin', 'x64'), '@relevate/poolster-cli-darwin-x64');
  assert.equal(platformPackage('win32', 'x64'), '@relevate/poolster-cli-win32-x64-msvc');
  assert.equal(platformPackage('linux', 'x64', { getReport: () => ({ header: { glibcVersionRuntime: '2.35' } }) }), '@relevate/poolster-cli-linux-x64-gnu');
  assert.throws(() => platformPackage('linux', 'x64', { getReport: () => ({ header: {} }) }), /musl/);
  assert.throws(() => platformPackage('linux', 'arm64'), /does not yet provide/);
});

test('forwards arguments and exit status to a native executable', () => {
  const launcher = path.resolve(__dirname, '../bin/poolster.cjs');
  const result = spawnSync(process.execPath, [launcher, '-e', 'process.exit(Number(process.argv[1]))', '17'], {
    encoding: 'utf8', env: { ...process.env, POOLSTER_BINARY: process.execPath },
  });
  assert.equal(result.status, 17, result.stderr);
});

test('missing executable produces a useful error and nonzero exit', () => {
  const result = spawnSync(process.execPath, [path.resolve(__dirname, '../bin/poolster.cjs')], {
    encoding: 'utf8', env: { ...process.env, POOLSTER_BINARY: path.join(__dirname, 'missing-executable') },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /could not start/);
});

test('routes JavaScript configs through the separately installed SDK', () => {
  const fs = require('node:fs');
  const os = require('node:os');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'poolster-cli-config-'));
  try {
    const sdk = path.join(directory, 'node_modules', '@relevate', 'poolster');
    fs.mkdirSync(sdk, { recursive: true });
    fs.writeFileSync(path.join(sdk, 'index.js'), `module.exports = {
      loadConfig: async (file) => ({ file }),
      generate: async (config, options) => {
        if (!config.file.endsWith('poolster.config.mjs') || options.write !== false) throw Error('wrong config dispatch');
        return { changes: { added: ['example.txt'], modified: [], removed: [] }, output: '/preview' };
      },
    };`);
    fs.writeFileSync(path.join(directory, 'poolster.config.mjs'), 'export default {};\n');
    const result = spawnSync(process.execPath, [path.resolve(__dirname, '../bin/poolster.cjs'), 'generate', '--dry-run'], {
      cwd: directory, encoding: 'utf8', env: { ...process.env, POOLSTER_BINARY: path.join(directory, 'missing-native') },
    });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /1 added, 0 modified, 0 removed in \/preview/);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('JavaScript config reports a missing SDK clearly', () => {
  const fs = require('node:fs');
  const os = require('node:os');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'poolster-cli-config-'));
  try {
    fs.writeFileSync(path.join(directory, 'poolster.config.cjs'), 'module.exports = {};\n');
    const result = spawnSync(process.execPath, [path.resolve(__dirname, '../bin/poolster.cjs'), 'generate'], {
      cwd: directory, encoding: 'utf8', env: { ...process.env, POOLSTER_BINARY: path.join(directory, 'missing-native') },
    });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /JS configs require @relevate\/poolster/);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
