'use strict';

function pluginRust(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginRust options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-rust', package: { ...options, language: 'rust', path: options.path ?? 'rust' } };
}

module.exports = { pluginRust };
