'use strict';

function pluginCSharp(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginCSharp options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-csharp', package: { ...options, language: 'csharp', path: options.path ?? 'csharp' } };
}

module.exports = { pluginCSharp };
