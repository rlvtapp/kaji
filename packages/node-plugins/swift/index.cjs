'use strict';

function pluginSwift(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginSwift options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-swift', package: { ...options, language: 'swift', path: options.path ?? 'swift' } };
}

module.exports = { pluginSwift };
