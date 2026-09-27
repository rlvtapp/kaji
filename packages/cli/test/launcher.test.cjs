'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const path = require('node:path');
const { platformPackage } = require('../bin/kaji.cjs');

test('resolves supported platforms without installing or downloading anything', () => {
  assert.equal(platformPackage('darwin', 'arm64'), '@relevate/kaji-darwin-arm64');
  assert.equal(platformPackage('darwin', 'x64'), '@relevate/kaji-darwin-x64');
  assert.equal(platformPackage('win32', 'x64'), '@relevate/kaji-win32-x64-msvc');
  assert.equal(platformPackage('linux', 'x64', { getReport: () => ({ header: { glibcVersionRuntime: '2.35' } }) }), '@relevate/kaji-linux-x64-gnu');
  assert.throws(() => platformPackage('linux', 'x64', { getReport: () => ({ header: {} }) }), /musl/);
  assert.throws(() => platformPackage('linux', 'arm64'), /does not yet provide/);
});

test('forwards arguments and exit status to a native executable', () => {
  const launcher = path.resolve(__dirname, '../bin/kaji.cjs');
  const result = spawnSync(process.execPath, [launcher, '-e', 'process.exit(Number(process.argv[1]))', '17'], {
    encoding: 'utf8', env: { ...process.env, KAJI_BINARY: process.execPath },
  });
  assert.equal(result.status, 17, result.stderr);
});

test('missing executable produces a useful error and nonzero exit', () => {
  const result = spawnSync(process.execPath, [path.resolve(__dirname, '../bin/kaji.cjs')], {
    encoding: 'utf8', env: { ...process.env, KAJI_BINARY: path.join(__dirname, 'missing-executable') },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /could not start/);
});
