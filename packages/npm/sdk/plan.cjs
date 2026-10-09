'use strict';
const { defineConfig, validatePlugin } = require('./config.cjs');
const { planPlugins } = require('./plugin-engine.cjs');

// Uses the execution planner, but never loads an input or runs a callback.
function plan(config) {
  defineConfig(config);
  const names = new Set();
  const js = [];
  const jsNames = new Set();
  for (const plugin of config.plugins) {
    const native = ['native-sdk', 'native-addon'].includes(plugin.kind);
    if ((native ? jsNames : names).has(plugin.name)) throw new Error(`duplicate plugin name ${plugin.name}`);
    names.add(plugin.name);
    if (!native) { js.push(validatePlugin(plugin)); jsNames.add(plugin.name); }
  }
  if (config.input.plugin) names.add(config.input.plugin.name);
  const resolved = planPlugins(js, names);
  const ids = new Map(config.plugins.map((plugin, id) => [plugin, id]));
  const plugins = config.plugins.map((plugin, id) => ({
    id, kind: plugin.name, phase: plugin.kind === 'native-sdk' || plugin.kind === 'native-addon' ? 'native-rendering' : plugin.phase ?? 'generate',
    order: resolved.order.includes(plugin) ? resolved.order.indexOf(plugin) : null,
    detail: js.includes(plugin) ? 'declared' : 'opaque-native-configuration',
    hooks: js.includes(plugin) ? ['transformApi', 'generate', 'schema', 'operation'].filter(key => typeof (plugin.hooks ?? plugin)[key] === 'function') : [],
    provides: (plugin.provides ?? []).map(c => c.name),
    requires: (plugin.requires ?? []).map(r => typeof r === 'string' ? r : r.contract.name),
  }));
  const edges = [];
  for (const plugin of js) {
    for (const req of plugin.requires ?? []) {
      const provider = typeof req === 'string' ? config.plugins.find(p => p.name === req) : resolved.bindings.get(plugin).get(req.contract);
      edges.push({ provider: provider ? ids.get(provider) ?? null : null, consumer: ids.get(plugin), contract: typeof req === 'string' ? null : req.contract.name,
        dependency: typeof req === 'string' ? req : null, optional: typeof req !== 'string' && req.optional,
        selection: typeof req === 'string' ? 'name' : req.from ? 'explicit' : 'automatic' });
    }
  }
  return { version: 1, runtime: 'javascript', input: { format: config.input.plugin?.format ?? 'openapi', provider: config.input.plugin?.name ?? 'openapi.compiler' }, plugins, edges,
    stages: ['input loading', 'HTTP transforms (HTTP inputs only)', 'native rendering and assembly', 'JavaScript Generate', 'JavaScript Post', 'ownership checks at check/write'],
    limitations: ['Native nodes are configured boundaries; their internal Rust graph is not exposed through the Node addon.', 'No callbacks run; conditional publications and actual files are unknown.'] };
}
function formatPlan(plan) {
  const lines = ['Poolster plan (declarations; no execution)', `input: ${plan.input.format} (${plan.input.provider})`];
  for (const phase of ['native-rendering', 'generate', 'post']) {
    lines.push(`[${phase}]`);
    for (const node of plan.plugins.filter(p => p.phase === phase).sort((a,b) => (a.order ?? a.id) - (b.order ?? b.id))) {
      lines.push(`  #${node.id} ${node.kind}${node.detail === 'opaque-native-configuration' ? ' [opaque native boundary]' : ''}`);
      for (const hook of node.hooks) lines.push(`     hook: ${hook}`);
      for (const edge of plan.edges.filter(e => e.consumer === node.id)) lines.push(`     <- ${edge.contract ?? edge.dependency} from ${edge.provider === null ? 'unbound' : '#'+edge.provider}`);
      for (const contract of node.provides) lines.push(`     -> ${contract}`);
    }
  }
  lines.push(`lifecycle: ${plan.stages.join(' -> ')}`);
  return lines.join('\n');
}
module.exports = { plan, formatPlan };
