'use strict';

function pluginCypress(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginCypress options must be an object');
  for (const key of Object.keys(options)) if (key !== 'target' && key !== 'output' && !(key === 'fixtureOptions' && ['faker','msw','cypress'].includes('cypress')) && !(key === 'cypressOptions' && 'cypress' === 'cypress')) throw new TypeError('unknown pluginCypress option ' + key);
  if (options.target != null && (typeof options.target !== 'string' || !options.target)) throw new TypeError('target must be a nonempty string');
  if (options.output != null && (typeof options.output !== 'string' || !options.output)) throw new TypeError('output must be a nonempty string');
  return { kind: 'native-addon', name: '@relevate/poolster-plugin-cypress', plugin: 'cypress', target: options.target ?? 'typescript', ...(options.output ? { output: options.output } : {}), ...(options.fixtureOptions !== undefined ? { fixtureOptions: options.fixtureOptions } : {}), ...(options.cypressOptions !== undefined ? { cypressOptions: options.cypressOptions } : {}) };
}

module.exports = { pluginCypress };
