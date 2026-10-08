#!/usr/bin/env node
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const root = path.resolve(packageRoot, '../..');
const key = process.argv[2];
if (!key || !/^(darwin-(arm64|x64)|linux-x64-gnu|win32-x64-msvc)$/.test(key)) {
  throw new Error('Supply a supported native platform directory');
}
const output = fs.mkdtempSync(path.join(os.tmpdir(), 'poolster-cli-smoke-'));
const environment = { ...process.env };
delete environment.POOLSTER_OPENAPI_BIN;
// Prove runtime generation does not require Cargo, rustc, Go, or other PATH tools.
for (const name of Object.keys(environment)) {
  if (name.toLowerCase() === 'path') delete environment[name];
}
environment.PATH = path.join(output, 'no-toolchains');
fs.mkdirSync(environment.PATH);
const result = spawnSync(process.execPath, [
  path.join(packageRoot, 'bin/poolster.cjs'), 'generate',
  path.join(root, 'crates/kaji-cli/tests/fixtures/pets.yaml'),
  '--output', output, '--language', 'go,typescript', '--jobs', '2',
], { stdio: 'inherit', env: {
  ...environment,
  POOLSTER_BINARY: path.join(packageRoot, 'npm', key, key.startsWith('win32') ? 'poolster.exe' : 'poolster'),
} });
if (result.error) throw result.error;
if (result.status !== 0) throw new Error(`Packaged CLI failed (${result.status ?? result.signal})`);
for (const target of ['go', 'typescript']) {
  if (!fs.statSync(path.join(output, target)).isDirectory()) throw new Error(`Missing ${target} SDK`);
}
console.log(`Packaged Rust CLI + adjacent Go compiler smoke passed: ${output}`);
