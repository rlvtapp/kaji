'use strict';

function pluginReactQuery(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginReactQuery options must be an object');
  for (const key of Object.keys(options)) if (key !== 'target' && key !== 'output') throw new TypeError('unknown pluginReactQuery option ' + key);
  if (options.target != null && (typeof options.target !== 'string' || !options.target)) throw new TypeError('target must be a nonempty string');
  if (options.output != null && (typeof options.output !== 'string' || !options.output)) throw new TypeError('output must be a nonempty string');
  return { kind: 'native-addon', name: '@relevate/poolster-plugin-react-query', plugin: 'react-query', target: options.target ?? 'typescript', ...(options.output ? { output: options.output } : {}) };
}

module.exports = { pluginReactQuery };
