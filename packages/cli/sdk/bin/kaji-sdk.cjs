#!/usr/bin/env node
'use strict';

const { loadConfig, generate } = require('../index.cjs');
const { version } = require('../package.json');

const usage = `Usage: kaji-sdk generate [--config <file>] [--check | --dry-run]

Loads kaji.config.mjs, kaji.config.js, or kaji.config.cjs from the current
directory. Every SDK and JavaScript generator must be listed in plugins.
`;

async function main(args = process.argv.slice(2)) {
  if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
    process.stdout.write(usage);
    return 0;
  }
  if (args.length === 1 && args[0] === '--version') {
    process.stdout.write(`${version}\n`);
    return 0;
  }
  if (args.shift() !== 'generate') throw new Error('Expected the generate command');
  let file;
  let check = false;
  let dryRun = false;
  while (args.length) {
    const flag = args.shift();
    if (flag === '--config') {
      file = args.shift();
      if (!file) throw new Error('--config requires a file');
    } else if (flag === '--check') {
      check = true;
    } else if (flag === '--dry-run') {
      dryRun = true;
    } else {
      throw new Error(`Unknown option ${flag}`);
    }
  }
  if (check && dryRun) throw new Error('Choose --check or --dry-run');
  const config = await loadConfig(file);
  const result = await generate(config, { write: !check && !dryRun });
  const { added, modified, removed } = result.changes;
  process.stdout.write(`${added.length} added, ${modified.length} modified, ${removed.length} removed in ${result.output}\n`);
  return check && (added.length || modified.length || removed.length) ? 1 : 0;
}

if (require.main === module) {
  main().then((code) => { process.exitCode = code; }).catch((error) => {
    console.error(`kaji-sdk: ${error.message}`);
    process.exitCode = 1;
  });
}

module.exports = { main };
