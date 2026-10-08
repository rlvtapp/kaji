'use strict';

function pluginElixir(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginElixir options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-elixir', package: { ...options, language: 'elixir', path: options.path ?? 'elixir' } };
}

module.exports = { pluginElixir };
