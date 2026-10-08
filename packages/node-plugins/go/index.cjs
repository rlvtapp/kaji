'use strict';

function pluginGo(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginGo options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-go', package: { ...options, language: 'go', path: options.path ?? 'go' } };
}

module.exports = { pluginGo };
