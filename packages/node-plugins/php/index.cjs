'use strict';

function pluginPhp(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginPhp options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-php', package: { ...options, language: 'php', path: options.path ?? 'php' } };
}

module.exports = { pluginPhp };
