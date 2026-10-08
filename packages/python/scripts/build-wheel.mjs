#!/usr/bin/env node
// Build-time tooling only. The published Python package never compiles or downloads binaries.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const root = path.resolve(packageRoot, '../..');
const key = process.argv[2];
const platforms = new Set(['darwin-arm64', 'darwin-x64', 'linux-x64-gnu', 'win32-x64-msvc']);
if (!platforms.has(key)) throw new Error(`Unsupported platform ${key}. Choose ${[...platforms].join(', ')}`);

const suffix = key.startsWith('win32') ? '.exe' : '';
const source = path.join(root, 'packages', 'cli', 'npm', key);
const destination = path.join(packageRoot, 'src', 'poolster', 'bin');
const dist = path.join(packageRoot, 'dist');
fs.rmSync(dist, { recursive: true, force: true });
fs.mkdirSync(dist, { recursive: true });
for (const binary of [`poolster${suffix}`, `poolster-openapi${suffix}`]) {
  const input = path.join(source, binary);
  if (!fs.existsSync(input)) throw new Error(`Missing ${input}. Build the native package first.`);
  fs.copyFileSync(input, path.join(destination, binary));
  if (!suffix) fs.chmodSync(path.join(destination, binary), 0o755);
}

const python = process.env.PYTHON ?? (process.platform === 'win32' ? 'python' : 'python3');
const result = spawnSync(
  python,
  ['-m', 'pip', 'wheel', '--no-deps', '--wheel-dir', 'dist', '.'],
  { cwd: packageRoot, stdio: 'inherit', env: { ...process.env, POOLSTER_PYTHON_PLATFORM: key } },
);
if (result.error) throw result.error;
if (result.status !== 0) throw new Error(`${python} -m pip wheel failed (${result.status ?? result.signal})`);
console.log(`Built a ${key} wheel in ${dist}. Nothing has been published.`);
