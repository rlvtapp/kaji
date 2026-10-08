'use strict';

const fs = require('node:fs');
const path = require('node:path');

const configNames = ['poolster.config.mjs', 'poolster.config.cjs', 'poolster.config.js'];

function jsConfig(args, cwd = process.cwd()) {
  if (args[0] !== 'generate') return null;
  const flag = args.indexOf('--config');
  if (flag >= 0) {
    const file = args[flag + 1];
    if (!file) throw new Error('--config requires a file');
    if (/\.[cm]?js$/i.test(file)) return path.resolve(cwd, file);
    if (/\.tsx?$/i.test(file)) throw new Error('TypeScript configs need a loader; use poolster.config.mjs or .cjs');
    return null;
  }
  const found = configNames.map((name) => path.resolve(cwd, name)).find((file) => fs.existsSync(file));
  return found ?? null;
}

function loadSdk(configFile) {
  if (process.env.POOLSTER_SDK_PACKAGE) {
    const override = path.resolve(process.env.POOLSTER_SDK_PACKAGE);
    return require(fs.statSync(override).isDirectory() ? path.join(override, 'index.cjs') : override);
  }
  let entry;
  try {
    entry = require.resolve('@relevate/poolster', { paths: [path.dirname(configFile), process.cwd()] });
  } catch {
    throw new Error('JS configs require @relevate/poolster. Install it alongside poolster.');
  }
  return require(entry);
}

async function runJsConfig(args, configFile) {
  if (!fs.existsSync(configFile)) throw new Error(`No Poolster config found: ${configFile}`);
  const sdk = loadSdk(configFile);
  let check = false;
  let dryRun = false;
  for (let i = 1; i < args.length; i++) {
    const flag = args[i];
    if (flag === '--config') { i++; continue; }
    if (flag === '--check') check = true;
    else if (flag === '--dry-run') dryRun = true;
    else throw new Error(`Unknown option ${flag}`);
  }
  if (check && dryRun) throw new Error('Choose --check or --dry-run');
  const config = await sdk.loadConfig(configFile);
  const result = await sdk.generate(config, { write: !check && !dryRun });
  const { added, modified, removed } = result.changes;
  process.stdout.write(`${added.length} added, ${modified.length} modified, ${removed.length} removed in ${result.output}\n`);
  return check && (added.length || modified.length || removed.length) ? 1 : 0;
}

module.exports = { jsConfig, loadSdk, runJsConfig };
