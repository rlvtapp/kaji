'use strict';

function create(language, name) {
  return (options = {}) => {
    if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError(`${name} options must be an object`);
    return { kind: 'native-sdk', name, package: { ...options, language, path: options.path ?? language } };
  };
}

function createAddon(plugin, name) {
  return (options = {}) => {
    if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError(`${name} options must be an object`);
    for (const key of Object.keys(options)) if (key !== 'target' && key !== 'output') throw new TypeError(`unknown ${name} option ${key}`);
    if (options.target != null && (typeof options.target !== 'string' || !options.target)) throw new TypeError('target must be a nonempty string');
    if (options.output != null && (typeof options.output !== 'string' || !options.output)) throw new TypeError('output must be a nonempty string');
    return { kind: 'native-addon', name, plugin, target: options.target ?? 'typescript', ...(options.output ? { output: options.output } : {}) };
  };
}

module.exports = {
  pluginTypeScript: create('typescript', '@relevate/kaji-plugin-typescript'),
  pluginRust: create('rust', '@relevate/kaji-plugin-rust'),
  pluginGo: create('go', '@relevate/kaji-plugin-go'),
  pluginPython: create('python', '@relevate/kaji-plugin-python'),
  pluginPhp: create('php', '@relevate/kaji-plugin-php'),
  pluginJava: create('java', '@relevate/kaji-plugin-java'),
  pluginCSharp: create('csharp', '@relevate/kaji-plugin-csharp'),
  pluginElixir: create('elixir', '@relevate/kaji-plugin-elixir'),
  pluginRuby: create('ruby', '@relevate/kaji-plugin-ruby'),
  pluginSwift: create('swift', '@relevate/kaji-plugin-swift'),
  pluginZod: createAddon('zod', '@relevate/kaji-plugin-zod'),
  pluginFaker: createAddon('faker', '@relevate/kaji-plugin-faker'),
  pluginMsw: createAddon('msw', '@relevate/kaji-plugin-msw'),
  pluginCypress: createAddon('cypress', '@relevate/kaji-plugin-cypress'),
  pluginReactQuery: createAddon('react-query', '@relevate/kaji-plugin-react-query'),
  pluginVueQuery: createAddon('vue-query', '@relevate/kaji-plugin-vue-query'),
  pluginSwr: createAddon('swr', '@relevate/kaji-plugin-swr'),
  inputGraphql: () => ({ kind: 'native-input', name: '@relevate/kaji-input-graphql', format: 'graphql', provider: 'graphql.apollo' }),
  inputAsyncApi: () => ({ kind: 'native-input', name: '@relevate/kaji-input-asyncapi', format: 'asyncapi', provider: 'asyncapi.roas' }),
  inputArazzo: () => ({ kind: 'native-input', name: '@relevate/kaji-input-arazzo', format: 'arazzo', provider: 'arazzo.roas' }),
  inputProtobuf: () => ({ kind: 'native-input', name: '@relevate/kaji-input-protobuf', format: 'protobuf', provider: 'protobuf.protox' }),
  inputCapnProto: () => ({ kind: 'native-input', name: '@relevate/kaji-input-capnproto', format: 'capnproto', provider: 'capnproto.capnp' }),
};
