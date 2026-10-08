'use strict';

function pluginJava(options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('pluginJava options must be an object');
  return { kind: 'native-sdk', name: '@relevate/kaji-plugin-java', package: { ...options, language: 'java', path: options.path ?? 'java' } };
}

module.exports = { pluginJava };
