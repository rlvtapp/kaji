import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const root = path.resolve(packageRoot, '../../..');
const result = spawnSync('cargo', ['build', '--locked', '-p', 'kaji-node'], { cwd: root, stdio: 'inherit' });
if (result.error) throw result.error;
if (result.status !== 0) throw new Error(`cargo build failed (${result.status ?? result.signal})`);
const library = process.platform === 'win32' ? 'kaji_node.dll' : process.platform === 'darwin' ? 'libkaji_node.dylib' : 'libkaji_node.so';
const source = path.join(root, 'target', 'debug', library);
const output = path.join(packageRoot, 'native', 'kaji_node.node');
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.copyFileSync(source, output);
console.log(`Built ${output}`);
