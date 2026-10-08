'use strict';

function pluginTypeScript(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginTypeScript options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-typescript', package: { ...options, language: 'typescript', path: options.path ?? 'typescript' } };
}

module.exports = { pluginTypeScript };
