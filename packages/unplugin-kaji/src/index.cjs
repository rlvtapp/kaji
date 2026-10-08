'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');

function asAbsolute(cwd, file) {
  return path.resolve(cwd, file);
}

function configWatchFiles(options, cwd) {
  if (typeof options.config !== 'string') return [];
  const config = asAbsolute(cwd, options.config);
  const files = [config];
  try {
    const source = JSON.parse(fs.readFileSync(config, 'utf8'));
    const input = source?.openapi?.input;
    if (typeof input === 'string' && !/^https?:\/\//i.test(input)) {
      files.push(path.resolve(path.dirname(config), input));
    }
  } catch {
    // The generator reports malformed or missing configuration with its normal
    // diagnostic. The config itself remains watched so a later fix reruns it.
  }
  return files;
}

function defaultArgs(options) {
  if (options.args) return [...options.args];
  if (typeof options.config !== 'string' || !options.config) {
    throw new Error('unplugin-kaji needs either `config` (the default is kaji.json) or explicit `args`.');
  }
  return ['generate', '--config', options.config];
}

function resolveLauncher(options) {
  if (options.command) return null;
  if (options.launcher) return path.resolve(options.launcher);
  try {
    return require.resolve('kajicli/bin/kaji.cjs');
  } catch {
    throw new Error('Cannot find kajicli. Install it alongside @relevate/unplugin-kaji, or set `command` to a Kaji executable.');
  }
}

function runKaji(options, cwd) {
  const args = defaultArgs(options);
  const launcher = resolveLauncher(options);
  const command = options.command || process.execPath;
  const commandArgs = launcher ? [launcher, ...args] : args;
  const env = { ...process.env, ...options.env };

  return new Promise((resolve, reject) => {
    const child = spawn(command, commandArgs, { cwd, env, windowsHide: true });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('error', (error) => reject(error));
    child.on('close', (code, signal) => {
      const result = { command, args: commandArgs, stdout, stderr, code, signal };
      if (code === 0 && !signal) return resolve(result);
      const detail = stderr.trim() || stdout.trim() || `Kaji exited with ${signal ? `signal ${signal}` : `status ${code}`}.`;
      const error = new Error(`Kaji generation failed: ${detail}`);
      error.result = result;
      reject(error);
    });
  });
}

function createController(rawOptions = {}) {
  const options = { config: 'kaji.json', watch: true, ...rawOptions };
  const cwd = path.resolve(options.cwd || process.cwd());
  const watchFiles = new Set();
  let current = Promise.resolve();
  let lastContext;

  function refreshWatchFiles() {
    for (const file of configWatchFiles(options, cwd)) watchFiles.add(file);
    for (const file of options.watchFiles || []) watchFiles.add(asAbsolute(cwd, file));
  }

  function register(context) {
    if (context) lastContext = context;
    refreshWatchFiles();
    if (typeof lastContext?.addWatchFile === 'function') {
      for (const file of watchFiles) lastContext.addWatchFile(file);
    }
  }

  function report(result) {
    if (!options.silent) {
      if (result.stdout) process.stdout.write(result.stdout);
      if (result.stderr) process.stderr.write(result.stderr);
    }
    options.onGenerate?.(result);
  }

  function generate() {
    const next = current.then(async () => {
      const result = await runKaji(options, cwd);
      // A recipe edit can point at a new source document. Add that document to
      // the bundler watcher immediately after the successful regeneration.
      register();
      report(result);
      return result;
    });
    // Keep later watch events usable after a failed generation while returning
    // the actual failure to the bundler that requested this run.
    current = next.catch(() => undefined);
    return next;
  }

  return {
    files: () => [...watchFiles],
    buildStart(context) {
      register(context);
      return generate();
    },
    watchChange(id, context) {
      if (!options.watch || !watchFiles.has(path.resolve(id))) return undefined;
      register(context);
      return generate();
    },
  };
}

function kaji(rawOptions = {}) {
  const controller = createController(rawOptions);
  return {
    name: 'kaji',
    enforce: 'pre',
    buildStart() {
      return controller.buildStart(this);
    },
    watchChange(id) {
      return controller.watchChange(id, this);
    },
  };
}

module.exports = kaji;
module.exports.kaji = kaji;
module.exports.createKajiPlugin = kaji;
module.exports.createController = createController;
module.exports.runKaji = runKaji;
