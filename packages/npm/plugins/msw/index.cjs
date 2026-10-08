'use strict';

function pluginMsw(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginMsw options must be an object');
  for (const key of Object.keys(options)) if (key !== 'target' && key !== 'output') throw new TypeError('unknown pluginMsw option ' + key);
  if (options.target != null && (typeof options.target !== 'string' || !options.target)) throw new TypeError('target must be a nonempty string');
  if (options.output != null && (typeof options.output !== 'string' || !options.output)) throw new TypeError('output must be a nonempty string');
  return { kind: 'native-addon', name: '@relevate/poolster-plugin-msw', plugin: 'msw', target: options.target ?? 'typescript', ...(options.output ? { output: options.output } : {}) };
}

module.exports = { pluginMsw };
