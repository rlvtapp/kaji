'use strict';

// @ts-check

const contractTag = Symbol('kaji.contract');
const handleTag = Symbol('kaji.providerHandle');

/**
 * A contract is a process-local token. Export the same token from a plugin
 * package so providers and consumers agree on its identity.
 * @param {string} name
 */
function defineContract(name) {
  if (typeof name !== 'string' || !name.trim()) throw new TypeError('contract name must be nonempty');
  return Object.freeze({ name, [contractTag]: true });
}

/** @param {unknown} value */
function isContract(value) {
  return !!value && typeof value === 'object' && value[contractTag] === true;
}

/**
 * Bind a consumer to one particular provider instance.
 * @param {object} provider
 * @param {object} contract
 */
function providerHandle(provider, contract) {
  if (!provider || typeof provider !== 'object') throw new TypeError('provider must be a plugin instance');
  if (!isContract(contract)) throw new TypeError('provider handle needs a contract from defineContract');
  return Object.freeze({ provider, contract, [handleTag]: true });
}

/**
 * Declare a typed dependency. An optional dependency may be absent, but an
 * ambiguous dependency still needs a provider handle.
 * @param {object} contract
 * @param {{ from?: object, optional?: boolean }} [options]
 */
function requireContract(contract, options = {}) {
  if (!isContract(contract)) throw new TypeError('requireContract needs a contract from defineContract');
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('requireContract options must be an object');
  if (options.from != null && (!options.from[handleTag] || options.from.contract !== contract)) {
    throw new TypeError(`provider handle does not select ${contract.name}`);
  }
  if (options.optional != null && typeof options.optional !== 'boolean') throw new TypeError('optional must be boolean');
  return Object.freeze({ contract, from: options.from, optional: options.optional === true });
}

/**
 * Resolve the same essential provider graph rules as Kaji's Rust package
 * engine, using contract tokens rather than Rust TypeId values.
 * @param {object[]} plugins
 * @param {Set<string>} nativeNames
 */
function planPlugins(plugins, nativeNames) {
  const byName = new Map(plugins.map((plugin) => [plugin.name, plugin]));
  const phase = new Map();
  const providers = new Map();
  const bindings = new Map();
  const dependencies = new Map();
  for (const plugin of plugins) {
    const currentPhase = plugin.phase ?? 'generate';
    if (currentPhase !== 'generate' && currentPhase !== 'post') throw new TypeError(`plugin ${plugin.name} phase must be generate or post`);
    if (currentPhase === 'post' && (plugin.hooks ?? plugin).transformApi) throw new Error(`post plugin ${plugin.name} cannot transform the API before generation`);
    phase.set(plugin, currentPhase);
    dependencies.set(plugin, new Set());
    const supplied = plugin.provides ?? [];
    if (!Array.isArray(supplied)) throw new TypeError(`plugin ${plugin.name}.provides must be an array`);
    const seen = new Set();
    for (const contract of supplied) {
      if (!isContract(contract)) throw new TypeError(`plugin ${plugin.name} provides an invalid contract`);
      if (seen.has(contract)) throw new Error(`plugin ${plugin.name} provides ${contract.name} twice`);
      seen.add(contract);
      if (!providers.has(contract)) providers.set(contract, []);
      providers.get(contract).push(plugin);
    }
  }
  for (const plugin of plugins) {
    const bound = new Map();
    for (const requirement of plugin.requires ?? []) {
      if (typeof requirement === 'string') {
        if (!nativeNames.has(requirement) && !byName.has(requirement)) {
          throw new Error(`plugin ${plugin.name} requires missing plugin ${requirement}`);
        }
        const dependency = byName.get(requirement);
        if (dependency) dependencies.get(plugin).add(dependency);
        continue;
      }
      if (!requirement || !isContract(requirement.contract)) throw new TypeError(`plugin ${plugin.name} has an invalid contract requirement`);
      const { contract, from, optional } = requirement;
      if (bound.has(contract)) throw new Error(`plugin ${plugin.name} requires ${contract.name} twice`);
      const candidates = providers.get(contract) ?? [];
      let selected;
      if (from) {
        if (from[handleTag] !== true || from.contract !== contract || !plugins.includes(from.provider)) {
          throw new Error(`plugin ${plugin.name} has an unregistered provider handle for ${contract.name}`);
        }
        if (!candidates.includes(from.provider)) throw new Error(`selected provider does not provide ${contract.name}`);
        selected = from.provider;
      } else if (candidates.length === 0) {
        if (!optional) throw new Error(`plugin ${plugin.name} requires ${contract.name}, but no provider is registered`);
      } else if (candidates.length === 1) {
        selected = candidates[0];
      } else {
        throw new Error(`plugin ${plugin.name} needs one ${contract.name} provider; found ${candidates.length}. Select a provider handle explicitly`);
      }
      bound.set(contract, selected);
      if (selected) dependencies.get(plugin).add(selected);
    }
    bindings.set(plugin, bound);
  }
  for (const plugin of plugins) {
    for (const dependency of dependencies.get(plugin)) {
      if (phase.get(plugin) === 'generate' && phase.get(dependency) === 'post') {
        throw new Error(`generate plugin ${plugin.name} cannot depend on post plugin ${dependency.name}`);
      }
    }
  }
  const sorted = [];
  const visiting = new Set();
  const visited = new Set();
  function visit(plugin) {
    if (visited.has(plugin)) return;
    if (visiting.has(plugin)) throw new Error(`JavaScript plugin dependency cycle at ${plugin.name}`);
    visiting.add(plugin);
    for (const dependency of dependencies.get(plugin)) visit(dependency);
    visiting.delete(plugin);
    visited.add(plugin);
    sorted.push(plugin);
  }
  for (const plugin of plugins) visit(plugin);
  return {
    order: [...sorted.filter((plugin) => phase.get(plugin) === 'generate'), ...sorted.filter((plugin) => phase.get(plugin) === 'post')],
    bindings,
    phase,
  };
}

/**
 * Contract values stay in memory within one generation, just like native
 * package-local publications. Nothing is serialized through NAPI.
 * @param {ReturnType<typeof planPlugins>} plan
 */
function contractRuntime(plan) {
  const publications = new Map();
  const workspace = new Map();
  return {
    context(plugin, base) {
      const supplied = new Set(plugin.provides ?? []);
      const bound = plan.bindings.get(plugin);
      const current = new Map();
      publications.set(plugin, current);
      return {
        ...base,
        workspace,
        inputs: {
          get(contract) {
            if (!bound.has(contract)) throw new Error(`plugin ${plugin.name} made an undeclared contract read: ${contract?.name ?? 'unknown contract'}`);
            const provider = bound.get(contract);
            if (!provider) throw new Error(`no provider bound for ${contract?.name ?? 'unknown contract'}`);
            if (!publications.get(provider)?.has(contract)) throw new Error(`provider ${provider.name} did not publish ${contract.name}`);
            return publications.get(provider).get(contract);
          },
          optional(contract) {
            if (!bound.has(contract)) throw new Error(`plugin ${plugin.name} made an undeclared contract read: ${contract?.name ?? 'unknown contract'}`);
            const provider = bound.get(contract);
            if (!provider) return undefined;
            if (!publications.get(provider)?.has(contract)) throw new Error(`provider ${provider.name} did not publish ${contract.name}`);
            return publications.get(provider).get(contract);
          },
        },
        publish(contract, value) {
          if (!supplied.has(contract)) throw new Error(`plugin ${plugin.name} made an undeclared contract publication: ${contract?.name ?? 'unknown contract'}`);
          if (current.has(contract)) throw new Error(`plugin ${plugin.name} published ${contract.name} twice`);
          current.set(contract, value);
        },
      };
    },
    complete(plugin) {
      const current = publications.get(plugin);
      for (const contract of plugin.provides ?? []) {
        if (!current.has(contract)) throw new Error(`plugin ${plugin.name} did not publish declared contract ${contract.name}`);
      }
    },
  };
}

module.exports = { defineContract, providerHandle, requireContract, isContract, planPlugins, contractRuntime };
