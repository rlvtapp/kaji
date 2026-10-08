import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const root = path.resolve(packageRoot, '../../..');
const manifest = JSON.parse(fs.readFileSync(path.join(packageRoot, 'package.json'), 'utf8'));
const targets = {
  'darwin-arm64': ['aarch64-apple-darwin', 'darwin', 'arm64'],
  'darwin-x64': ['x86_64-apple-darwin', 'darwin', 'x64'],
  'linux-x64-gnu': ['x86_64-unknown-linux-gnu', 'linux', 'x64'],
  'win32-x64-msvc': ['x86_64-pc-windows-msvc', 'win32', 'x64'],
};
const key = process.argv[2];
if (!targets[key]) throw new Error(`Choose a platform: ${Object.keys(targets).join(', ')}`);
const [target, os, cpu] = targets[key];
const environment = os === 'darwin' ? { MACOSX_DEPLOYMENT_TARGET: key === 'darwin-arm64' ? '11.0' : '10.13' } : {};
const output = path.join(packageRoot, 'npm', key);
const compilerName = os === 'win32' ? 'kaji-openapi.exe' : 'kaji-openapi';
const library = os === 'win32' ? 'kaji_node.dll' : os === 'darwin' ? 'libkaji_node.dylib' : 'libkaji_node.so';

function run(command, args, cwd, extraEnv = {}) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit', env: { ...process.env, ...extraEnv } });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status ?? result.signal})`);
}

run('cargo', ['build', '--locked', '--release', '-p', 'kaji-node', '--target', target], root, environment);
fs.mkdirSync(output, { recursive: true });
run('go', ['build', '-trimpath', '-ldflags=-s -w', '-o', path.join(output, compilerName), '.'],
  path.join(root, 'openapi'), { ...environment, GOOS: os === 'win32' ? 'windows' : os, GOARCH: cpu === 'x64' ? 'amd64' : cpu, CGO_ENABLED: '0' });
fs.copyFileSync(path.join(root, 'target', target, 'release', library), path.join(output, 'kaji_node.node'));
if (os !== 'win32') fs.chmodSync(path.join(output, compilerName), 0o755);
fs.copyFileSync(path.join(root, 'LICENSE'), path.join(output, 'LICENSE'));
const nativeManifest = {
  name: `@relevate/kaji-${key}`, version: manifest.version,
  description: `Native Kaji SDK runtime for ${key}`, license: manifest.license,
  repository: manifest.repository, os: [os], cpu: [cpu],
  ...(os === 'linux' ? { libc: ['glibc'] } : {}),
  files: ['kaji_node.node', compilerName, 'LICENSE'], publishConfig: { access: 'public' },
};
fs.writeFileSync(path.join(output, 'package.json'), `${JSON.stringify(nativeManifest, null, 2)}\n`);
console.log(`Built ${nativeManifest.name}@${manifest.version} in ${output}. Nothing has been published.`);
