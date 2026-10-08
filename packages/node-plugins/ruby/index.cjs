'use strict';

function pluginRuby(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginRuby options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-ruby', package: { ...options, language: 'ruby', path: options.path ?? 'ruby' } };
}

module.exports = { pluginRuby };
