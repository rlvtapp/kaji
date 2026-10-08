import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const platform = process.argv[2];
if (!platform) throw new Error('Expected a platform key');
const rootManifest = JSON.parse(fs.readFileSync(path.join(packageRoot, 'package.json'), 'utf8'));
const nativeRoot = path.join(packageRoot, '..', 'platform', 'node', platform);
const nativeManifest = JSON.parse(fs.readFileSync(path.join(nativeRoot, 'package.json'), 'utf8'));
if (nativeManifest.name !== `@relevate/poolster-node-${platform}` || nativeManifest.version !== rootManifest.version) {
  throw new Error('Native package metadata does not match the Node package');
}
const binding = require(path.join(nativeRoot, 'poolster_node.node'));
for (const name of ['loadContract', 'generateSdk', 'materialize', 'availableNativePlugins', 'availableInputPlugins', 'inspectInput']) {
  if (typeof binding[name] !== 'function') throw new Error(`Native addon is missing ${name}`);
}
console.log(`Loaded ${nativeManifest.name}`);
