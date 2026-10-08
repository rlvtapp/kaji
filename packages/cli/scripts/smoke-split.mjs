#!/usr/bin/env node
// Install both npm products from local tarballs to catch accidental cross-dependencies.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const platform = process.argv[2];
if (!platform || !/^(darwin-(arm64|x64)|linux-x64-gnu|win32-x64-msvc)$/.test(platform)) {
  throw new Error('Supply a supported platform key');
}
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'poolster-npm-split-'));
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';

function run(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd, encoding: 'utf8', shell: process.platform === 'win32',
    env: { ...process.env, npm_config_cache: path.join(temporary, 'npm-cache') },
  });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed:\n${result.stderr || result.error || result.stdout}`);
  }
  return result.stdout.trim();
}

function pack(relativePath) {
  const directory = path.join(root, relativePath);
  const filename = run(npm, ['pack', '--ignore-scripts', '--pack-destination', temporary], directory)
    .split(/\r?\n/).at(-1);
  return path.join(temporary, filename);
}

const cli = pack('packages/cli');
const cliNative = pack(`packages/cli/npm/${platform}`);
const sdk = pack('packages/cli/sdk');
const sdkNative = pack(`packages/cli/sdk/npm/${platform}`);
const cliConsumer = path.join(temporary, 'cli-consumer');
const sdkConsumer = path.join(temporary, 'sdk-consumer');
fs.mkdirSync(cliConsumer);
fs.mkdirSync(sdkConsumer);
const installFlags = ['install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', '--omit=optional'];
run(npm, [...installFlags, cli, cliNative], cliConsumer);
run(npm, [...installFlags, sdk, sdkNative], sdkConsumer);

if (fs.existsSync(path.join(cliConsumer, 'node_modules', '@relevate', 'poolster'))) {
  throw new Error('CLI-only install unexpectedly contains the SDK');
}
if (fs.existsSync(path.join(sdkConsumer, 'node_modules', 'poolster'))) {
  throw new Error('SDK-only install unexpectedly contains the CLI');
}
const version = run(process.execPath, [path.join(cliConsumer, 'node_modules', 'poolster', 'bin', 'poolster.cjs'), '--version'], cliConsumer);
if (!/^poolster \d+\.\d+\.\d+/.test(version)) throw new Error(`Unexpected CLI version output: ${version}`);

const fixture = path.join(root, 'crates/kaji-cli/tests/fixtures/pets.yaml');
const sdkProgram = `
  const { defineConfig, createPoolster } = require('@relevate/poolster/sdk');
  const { pluginTypeScript } = require('@relevate/poolster/sdk/plugins');
  const config = defineConfig({ input: process.argv[1], output: './generated', name: 'Pets', version: '1.0.0', plugins: [pluginTypeScript()] });
  createPoolster(config).generate({ write: false }).then(result => {
    if (!result.files.length) throw new Error('SDK generated no files');
    console.log(result.files.length + ' files');
  }).catch(error => { console.error(error); process.exitCode = 1; });
`;
const generated = run(process.execPath, ['-e', sdkProgram, fixture], sdkConsumer);
console.log(`CLI ${version}; SDK ${generated}; packages remain separate`);
