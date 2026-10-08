'use strict';

function pluginPython(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginPython options must be an object');
  return { kind: 'native-sdk', name: '@relevate/poolster-plugin-python', package: { ...options, language: 'python', path: options.path ?? 'python' } };
}

module.exports = { pluginPython };
