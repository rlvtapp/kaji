#!/usr/bin/env node
// Build-time tooling only. Published packages never run Cargo or Go.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const root = path.resolve(packageRoot, '../..');
const manifest = JSON.parse(fs.readFileSync(path.join(packageRoot, 'package.json'), 'utf8'));
const platforms = {
  'darwin-arm64': ['aarch64-apple-darwin', 'darwin', 'arm64'],
  'darwin-x64': ['x86_64-apple-darwin', 'darwin', 'amd64'],
  'linux-x64-gnu': ['x86_64-unknown-linux-gnu', 'linux', 'amd64'],
  'win32-x64-msvc': ['x86_64-pc-windows-msvc', 'windows', 'amd64'],
};
const key = process.argv[2] ?? `${process.platform}-${process.arch}${process.platform === 'linux' ? '-gnu' : process.platform === 'win32' ? '-msvc' : ''}`;
if (!platforms[key]) throw new Error(`Unsupported platform ${key}. Choose ${Object.keys(platforms).join(', ')}`);
const [target, goos, goarch] = platforms[key];
const output = path.join(packageRoot, 'npm', key);
const exe = goos === 'windows' ? '.exe' : '';
const cargoDirectory = path.join(root, 'target');
const platformEnvironment = goos === 'darwin'
  ? { MACOSX_DEPLOYMENT_TARGET: key === 'darwin-arm64' ? '11.0' : '10.13' }
  : {};

function run(command, args, cwd, extraEnv = {}) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit', env: { ...process.env, ...extraEnv } });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status ?? result.signal})`);
}

run('cargo', ['build', '--locked', '--release', '-p', 'kaji-cli', '--target', target, '--target-dir', cargoDirectory], root, platformEnvironment);
fs.mkdirSync(output, { recursive: true });
run('go', ['build', '-trimpath', '-ldflags=-s -w', '-o', path.join(output, `kaji-openapi${exe}`), '.'], path.join(root, 'openapi'), { ...platformEnvironment, GOOS: goos, GOARCH: goarch, CGO_ENABLED: '0' });
fs.copyFileSync(path.join(cargoDirectory, target, 'release', `kaji${exe}`), path.join(output, `kaji${exe}`));
for (const binary of [`kaji${exe}`, `kaji-openapi${exe}`]) if (!exe) fs.chmodSync(path.join(output, binary), 0o755);
fs.copyFileSync(path.join(root, 'LICENSE'), path.join(output, 'LICENSE'));
const nativeManifest = {
  name: `@relevate/kaji-${key}`, version: manifest.version,
  description: `Native Kaji executables for ${key}`, license: manifest.license,
  repository: manifest.repository, os: [key.split('-')[0]], cpu: [key.split('-')[1]],
  ...(goos === 'linux' ? { libc: ['glibc'] } : {}),
  files: [`kaji${exe}`, `kaji-openapi${exe}`, 'LICENSE'], publishConfig: { access: 'public' },
};
fs.writeFileSync(path.join(output, 'package.json'), `${JSON.stringify(nativeManifest, null, 2)}\n`);
console.log(`Built ${nativeManifest.name}@${manifest.version} in ${output}. Nothing has been published.`);
