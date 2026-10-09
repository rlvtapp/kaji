'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

function validateInput(input) {
  if (typeof input === 'string') return;
  if (input && typeof input === 'object' && typeof input.artifacts === 'string') return;
  if (input && typeof input === 'object' && typeof input.path === 'string' && input.path &&
      input.plugin?.kind === 'native-input' && typeof input.plugin.format === 'string' && input.plugin.format.trim() &&
      typeof input.plugin.provider === 'string' && input.plugin.provider.trim()) return;
  if (input && typeof input === 'object' && typeof input.path === 'string' && input.path &&
      input.plugin?.kind === 'js-input' && typeof input.plugin.name === 'string' && input.plugin.name.trim() &&
      typeof input.plugin.format === 'string' && input.plugin.format.trim() &&
      typeof input.plugin.load === 'function') return;
  throw new TypeError('input must be an OpenAPI path, { artifacts: directory }, or { path, plugin: inputPlugin() }');
}

function defineInputPlugin(factory) {
  if (typeof factory !== 'function') throw new TypeError('input plugin factory must be a function');
  return (...args) => {
    const plugin = factory(...args);
    validateInput({ path: '.', plugin });
    if (plugin.kind !== 'js-input') throw new TypeError('defineInputPlugin requires kind: js-input');
    return plugin;
  };
}

function defineConfig(config) {
  if (!config || typeof config !== 'object' || Array.isArray(config)) {
    throw new TypeError('Poolster config must be an object');
  }
  validateInput(config.input);
  if (config.input.plugin?.format === 'graphql') {
    if (config.input.operations != null && (!Array.isArray(config.input.operations) || config.input.operations.some((file) => typeof file !== 'string' || !file))) throw new TypeError('GraphQL operations must be nonempty file paths');
    if (config.input.subscriptions != null && typeof config.input.subscriptions !== 'boolean') throw new TypeError('GraphQL subscriptions must be boolean');
  }
  if (!config.input.plugin && (typeof config.name !== 'string' || !config.name.trim())) throw new TypeError('name is required');
  if (!config.input.plugin && (typeof config.version !== 'string' || !config.version.trim())) throw new TypeError('version is required');
  const destination = typeof config.output === 'string' ? config.output : config.output?.path;
  if (typeof destination !== 'string' || !destination.trim()) throw new TypeError('output or output.path is required');
  if (!Array.isArray(config.plugins) || config.plugins.length === 0) {
    throw new TypeError('plugins must contain at least one SDK or JavaScript plugin');
  }
  return config;
}

function sdkPackage(options) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) {
    throw new TypeError('sdk options must be an object');
  }
  if (typeof options.language !== 'string' || !options.language) {
    throw new TypeError('sdk language is required');
  }
  const allowed = new Set(['language', 'path', 'name', 'version', 'style', 'transport', 'clientName', 'raw', 'jobs']);
  for (const key of Object.keys(options)) {
    if (!allowed.has(key)) throw new TypeError(`unknown sdk option ${key}`);
  }
  if (!['typescript', 'rust', 'go', 'python', 'php', 'java', 'csharp', 'elixir', 'ruby', 'swift'].includes(options.language)) {
    throw new TypeError(`unsupported SDK language ${options.language}`);
  }
  const { language, path: outputPath = language, name, version, style, transport, clientName, raw, jobs } = options;
  if (typeof outputPath !== 'string' || !outputPath) throw new TypeError('sdk path must be a nonempty string');
  return { language, path: outputPath, name, version, style, transport, clientName, raw, jobs };
}

function normalizedPackagePath(value) {
  const normalized = path.posix.normalize(value.replaceAll('\\', '/'));
  return normalized.endsWith('/') && normalized !== '/' ? normalized.slice(0, -1) : normalized;
}

function definePlugin(factory) {
  if (typeof factory !== 'function') throw new TypeError('plugin factory must be a function');
  return (...args) => {
    const plugin = factory(...args);
    validatePlugin(plugin);
    return plugin;
  };
}

function validatePlugin(plugin) {
  if (!plugin || typeof plugin !== 'object' || typeof plugin.name !== 'string' || !plugin.name.trim()) {
    throw new TypeError('JavaScript plugins need a nonempty name');
  }
  const hooks = plugin.hooks ?? plugin;
  if (!hooks || typeof hooks !== 'object') throw new TypeError(`plugin ${plugin.name}.hooks must be an object`);
  if (!['transformApi', 'generate', 'schema', 'operation'].some((hook) => typeof hooks[hook] === 'function')) {
    throw new TypeError(`plugin ${plugin.name} needs transformApi, generate, schema, or operation`);
  }
  for (const hook of ['transformApi', 'generate', 'schema', 'operation']) {
    if (hooks[hook] != null && typeof hooks[hook] !== 'function') {
      throw new TypeError(`plugin ${plugin.name}.${hook} must be a function`);
    }
  }
  if (plugin.requires != null && !Array.isArray(plugin.requires)) {
    throw new TypeError(`plugin ${plugin.name}.requires must be an array`);
  }
  return plugin;
}

async function loadConfig(configFile) {
  const candidates = configFile
    ? [path.resolve(configFile)]
    : ['poolster.config.mjs', 'poolster.config.js', 'poolster.config.cjs'].map((file) => path.resolve(file));
  const file = candidates.find((candidate) => fs.existsSync(candidate));
  if (!file) throw new Error(`No Poolster config found. Looked for ${candidates.join(', ')}`);
  const module = await import(pathToFileURL(file).href);
  let config = module.default ?? module.config;
  if (typeof config === 'function') config = config();
  config = await config;
  defineConfig(config);
  const base = path.dirname(file);
  const resolve = (value) => path.resolve(base, value);
  return {
    ...config,
    input: typeof config.input === 'string' ? resolve(config.input) : config.input.plugin
      ? { ...config.input, path: resolve(config.input.path), ...(config.input.operations ? { operations: config.input.operations.map(resolve) } : {}) }
      : { artifacts: resolve(config.input.artifacts) },
    output: typeof config.output === 'string' ? resolve(config.output) : { ...config.output, path: resolve(config.output.path) },
    ...(config.compiler ? { compiler: resolve(config.compiler) } : {}),
  };
}

module.exports = { validateInput, defineInputPlugin, defineConfig, sdkPackage, normalizedPackagePath, definePlugin, validatePlugin, loadConfig };
