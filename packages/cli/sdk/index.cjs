'use strict';

const fs = require('node:fs');
const fsp = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { defineContract, providerHandle, requireContract, planPlugins, contractRuntime } = require('./plugin-engine.cjs');
const { validateInput, defineInputPlugin, defineConfig, sdkPackage, normalizedPackagePath, definePlugin, validatePlugin, loadConfig } = require('./config.cjs');

const root = path.resolve(__dirname, '../../..');

function platformKey(platform = process.platform, arch = process.arch, report = process.report) {
  if (platform === 'linux' && arch === 'x64') {
    if (!report?.getReport().header.glibcVersionRuntime) {
      throw new Error('Poolster native packages currently require glibc on Linux.');
    }
    return 'linux-x64-gnu';
  }
  const key = `${platform}-${arch}`;
  if (key === 'darwin-arm64' || key === 'darwin-x64') return key;
  if (key === 'win32-x64') return 'win32-x64-msvc';
  throw new Error(`Poolster has no native Node package for ${key}.`);
}

function resolveNative() {
  if (process.env.POOLSTER_NODE_BINARY) return path.resolve(process.env.POOLSTER_NODE_BINARY);
  const local = path.join(__dirname, 'native', 'poolster_node.node');
  if (fs.existsSync(local)) return local;
  const name = `@relevate/poolster-node-${platformKey()}`;
  let manifestPath;
  try {
    manifestPath = require.resolve(`${name}/package.json`);
  } catch {
    throw new Error(`Missing native package ${name}. Reinstall @relevate/poolster with optional dependencies enabled.`);
  }
  const installed = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  if (installed.version !== require('./package.json').version) {
    throw new Error(`Native package ${name} version ${installed.version} does not match @relevate/poolster.`);
  }
  return path.join(path.dirname(manifestPath), 'poolster_node.node');
}

let binding;
function native() {
  if (!binding) binding = require(resolveNative());
  return binding;
}

function availableNativePlugins() {
  return native().availableNativePlugins();
}

function availableInputPlugins() {
  return JSON.parse(native().availableInputPlugins());
}

async function loadInput(input) {
  validateInput(input);
  if (typeof input === 'string' || input.artifacts) throw new TypeError('inspectInput requires a selected input plugin');
  const source = path.resolve(input.path);
  if (input.plugin.kind === 'native-input') {
    return { report: JSON.parse(await native().inspectInput(input.plugin.format, input.plugin.provider, source)), api: null, securitySchemes: {} };
  }
  const loaded = await input.plugin.load(source);
  if (!loaded || typeof loaded !== 'object' || !loaded.summary || loaded.summary.format !== input.plugin.format ||
      typeof loaded.summary.title !== 'string' || !Array.isArray(loaded.summary.types) ||
      !Array.isArray(loaded.summary.operations) ||
      (loaded.diagnostics != null && !Array.isArray(loaded.diagnostics))) {
    throw new TypeError(`input plugin ${input.plugin.name} returned an invalid summary`);
  }
  if (loaded.api && (!Array.isArray(loaded.api.operations) || !Array.isArray(loaded.api.schemas))) {
    throw new TypeError(`input plugin ${input.plugin.name} returned an invalid HTTP API`);
  }
  return {
    report: { provider: input.plugin.name, source, summary: loaded.summary, diagnostics: loaded.diagnostics ?? [], data: loaded.data },
    api: loaded.api ?? null,
    securitySchemes: loaded.securitySchemes ?? {},
  };
}

async function inspectInput(input) {
  return (await loadInput(input)).report;
}

function resolveCompiler(config) {
  if (config.compiler) return path.resolve(config.compiler);
  if (process.env.POOLSTER_OPENAPI_BIN) return path.resolve(process.env.POOLSTER_OPENAPI_BIN);
  const exe = process.platform === 'win32' ? 'poolster-openapi.exe' : 'poolster-openapi';
  const local = path.join(root, 'target', 'debug', exe);
  if (fs.existsSync(local)) return local;
  const name = `@relevate/poolster-node-${platformKey()}`;
  let manifestPath;
  try {
    manifestPath = require.resolve(`${name}/package.json`);
  } catch {
    throw new Error(`Missing OpenAPI compiler ${name}. Install @relevate/poolster or set POOLSTER_OPENAPI_BIN.`);
  }
  const installed = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  if (installed.version !== require('./package.json').version) {
    throw new Error(`OpenAPI compiler package ${name} version does not match @relevate/poolster.`);
  }
  return path.join(path.dirname(manifestPath), exe);
}

function outputPath(value) {
  if (typeof value !== 'string' || !value || value.includes('\\') || path.isAbsolute(value) || path.win32.isAbsolute(value)) {
    throw new Error('generated file paths must be relative and use forward slashes');
  }
  if (value.split('/').some((part) => !part || part === '.' || part === '..')) {
    throw new Error(`unsafe generated file path ${JSON.stringify(value)}`);
  }
  return value;
}

function makeFileStore(nativeFiles) {
  const files = new Map();
  const caseFoldedPaths = new Set();
  for (const file of nativeFiles) {
    const key = outputPath(file.path.replaceAll('\\', '/').replace(/^(\.\/)+/, ''));
    if (caseFoldedPaths.has(key.toLowerCase())) throw new Error(`duplicate generated file ${key}`);
    files.set(key, { ...file, path: key });
    caseFoldedPaths.add(key.toLowerCase());
  }
  return { files, caseFoldedPaths };
}

function pluginContext(plugin, contract, files, caseFoldedPaths, output) {
  return {
    api: contract.api,
    input: contract.input,
    securitySchemes: contract.securitySchemes,
    output,
    emitFile(file) {
      if (!file || typeof file.contents !== 'string') throw new TypeError('emitFile requires string contents');
      const key = outputPath(file.path);
      if (caseFoldedPaths.has(key.toLowerCase())) {
        throw new Error(`plugin ${plugin.name} emitted an existing file: ${key}`);
      }
      files.set(key, {
        path: key,
        contents: file.contents,
        preserveExisting: file.preserveExisting === true,
        owner: `js-plugin:${plugin.name}`,
      });
      caseFoldedPaths.add(key.toLowerCase());
    },
    readFile(filePath) {
      return files.get(outputPath(filePath))?.contents;
    },
    replaceFile(filePath, contents) {
      if (typeof contents !== 'string') throw new TypeError('replaceFile requires string contents');
      const key = outputPath(filePath);
      const existing = files.get(key);
      if (!existing) throw new Error(`plugin ${plugin.name} cannot replace missing file: ${key}`);
      files.set(key, { ...existing, contents });
    },
  };
}

function compile(input, artifacts, compiler) {
  return new Promise((resolve, reject) => {
    const child = spawn(compiler, ['--out', artifacts, input], { stdio: ['ignore', 'ignore', 'pipe'], windowsHide: true });
    let stderr = '';
    let settled = false;
    child.stderr.on('data', (chunk) => { stderr = (stderr + chunk.toString()).slice(-65536); });
    child.on('error', (error) => { settled = true; reject(error); });
    child.on('close', (code, signal) => {
      if (settled) return;
      if (code === 0) resolve();
      else reject(new Error(`Poolster OpenAPI compiler failed (${signal || code}): ${stderr.trim()}`));
    });
  });
}

async function generate(config, options = {}) {
  defineConfig(config);
  if (options.write != null && typeof options.write !== 'boolean') throw new TypeError('write must be boolean');
  const plugins = config.plugins;
  const packages = [];
  const nativeAddons = [];
  const jsPluginsUnordered = [];
  const names = new Set();
  const jsNames = new Set();
  if (config.input.plugin) names.add(config.input.plugin.name);
  for (const plugin of plugins) {
    if (plugin?.kind === 'native-sdk') {
      if (typeof plugin.name !== 'string' || !plugin.name) throw new TypeError('native plugins need a name');
      if (jsNames.has(plugin.name)) throw new Error(`duplicate plugin name ${plugin.name}`);
      packages.push(sdkPackage(plugin.package));
      names.add(plugin.name);
    } else if (plugin?.kind === 'native-addon') {
      if (typeof plugin.name !== 'string' || !plugin.name) throw new TypeError('native plugins need a name');
      if (typeof plugin.target !== 'string' || !plugin.target) throw new TypeError(`native plugin ${plugin.name} needs a target package path`);
      if (typeof plugin.plugin !== 'string' || !plugin.plugin) throw new TypeError(`native plugin ${plugin.name} needs a registered plugin id`);
      if (plugin.output != null && (typeof plugin.output !== 'string' || !plugin.output)) throw new TypeError(`native plugin ${plugin.name} output must be a nonempty string`);
      if (jsNames.has(plugin.name)) throw new Error(`duplicate plugin name ${plugin.name}`);
      nativeAddons.push(plugin);
      names.add(plugin.name);
    } else {
      const validated = validatePlugin(plugin);
      if (names.has(plugin.name)) throw new Error(`duplicate plugin name ${plugin.name}`);
      names.add(plugin.name);
      jsNames.add(plugin.name);
      jsPluginsUnordered.push(validated);
    }
  }
  const packagePaths = new Set();
  for (const sdk of packages) {
    const normalized = normalizedPackagePath(sdk.path).toLowerCase();
    if (packagePaths.has(normalized)) throw new Error(`SDK package path ${sdk.path} is configured twice`);
    packagePaths.add(normalized);
  }
  for (const addon of nativeAddons) {
    const targetPath = normalizedPackagePath(addon.target);
    const targets = packages.filter((candidate) => normalizedPackagePath(candidate.path) === targetPath);
    if (targets.length !== 1) throw new Error(`native plugin ${addon.name} needs exactly one SDK package at ${addon.target}`);
    const target = targets[0];
    if (target.language !== 'typescript') throw new Error(`native plugin ${addon.name} requires a TypeScript SDK package`);
    if (target.plugins?.some((plugin) => plugin.name === addon.plugin)) throw new Error(`native plugin ${addon.plugin} is configured twice for ${addon.target}`);
    (target.plugins ??= []).push({ name: addon.plugin, ...(addon.output ? { output: addon.output } : {}) });
  }
  const plan = planPlugins(jsPluginsUnordered, names);
  const jsPlugins = plan.order;
  const output = path.resolve(typeof config.output === 'string' ? config.output : config.output.path);
  let temporary;
  try {
    let contract;
    if (config.input.plugin) {
      const loaded = await loadInput(config.input);
      if (!loaded.api && (packages.length || nativeAddons.length)) {
        throw new Error(`Input ${config.input.plugin.format} did not publish poolster.http-api, which the selected SDK plugins require`);
      }
      if (!loaded.api && jsPlugins.length === 0) throw new Error('native input generation needs a JavaScript output plugin');
      contract = { api: loaded.api, securitySchemes: loaded.securitySchemes, input: loaded.report };
    } else {
      let artifacts;
      if (typeof config.input === 'string') {
        temporary = await fsp.mkdtemp(path.join(os.tmpdir(), 'poolster-node-'));
        artifacts = temporary;
        await compile(path.resolve(config.input), artifacts, resolveCompiler(config));
      } else {
        artifacts = path.resolve(config.input.artifacts);
      }
      contract = JSON.parse(await native().loadContract(artifacts, config.name, config.version));
    }
    for (const plugin of jsPlugins) {
      const hooks = plugin.hooks ?? plugin;
      if (hooks.transformApi) {
        if (!contract.api) throw new Error(`plugin ${plugin.name} transforms an HTTP API, which input ${config.input.plugin.format} does not publish`);
        const replacement = await hooks.transformApi(contract.api, {
          securitySchemes: contract.securitySchemes,
          output,
        });
        if (replacement !== undefined) contract.api = replacement;
        if (!contract.api || !Array.isArray(contract.api.operations) || !Array.isArray(contract.api.schemas)) {
          throw new TypeError(`plugin ${plugin.name} returned an invalid API`);
        }
      }
    }
    const nativeFiles = contract.api ? JSON.parse(await native().generateSdk(JSON.stringify(contract), JSON.stringify(packages))) : [];
    const { files, caseFoldedPaths } = makeFileStore(nativeFiles);
    const contracts = contractRuntime(plan);
    for (const phase of ['generate', 'post']) {
      for (const plugin of jsPlugins) {
        if (plan.phase.get(plugin) !== phase) continue;
        const context = contracts.context(plugin, pluginContext(plugin, contract, files, caseFoldedPaths, output));
        const hooks = plugin.hooks ?? plugin;
        if (hooks.generate) await hooks.generate(context);
        if (hooks.schema && contract.api) {
          for (const schema of contract.api.schemas) await hooks.schema(schema, context);
        }
        if (hooks.operation && contract.api) {
          for (const operation of contract.api.operations) await hooks.operation(operation, context);
        }
        contracts.complete(plugin);
      }
    }
    const rendered = [...files.values()];
    const rawChanges = JSON.parse(await native().materialize(JSON.stringify(rendered), output, options.write !== false));
    const changes = Object.fromEntries(
      ['added', 'modified', 'removed'].map((kind) => [kind, rawChanges[kind].map((file) => file.replaceAll('\\', '/'))]),
    );
    return { api: contract.api, input: contract.input, files: rendered, changes, output };
  } finally {
    if (temporary) await fsp.rm(temporary, { recursive: true, force: true });
  }
}

function createPoolster(config) {
  defineConfig(config);
  return { generate: (options) => generate(config, options), inspectInput: () => inspectInput(config.input) };
}

module.exports = { defineConfig, definePlugin, defineInputPlugin, defineContract, providerHandle, requireContract, availableNativePlugins, availableInputPlugins, inspectInput, loadConfig, createPoolster, generate };
