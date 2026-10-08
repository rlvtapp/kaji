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
  pluginTypeScript: create('typescript', '@relevate/poolster-plugin-typescript'),
  pluginRust: create('rust', '@relevate/poolster-plugin-rust'),
  pluginGo: create('go', '@relevate/poolster-plugin-go'),
  pluginPython: create('python', '@relevate/poolster-plugin-python'),
  pluginPhp: create('php', '@relevate/poolster-plugin-php'),
  pluginJava: create('java', '@relevate/poolster-plugin-java'),
  pluginCSharp: create('csharp', '@relevate/poolster-plugin-csharp'),
  pluginElixir: create('elixir', '@relevate/poolster-plugin-elixir'),
  pluginRuby: create('ruby', '@relevate/poolster-plugin-ruby'),
  pluginSwift: create('swift', '@relevate/poolster-plugin-swift'),
  pluginZod: createAddon('zod', '@relevate/poolster-plugin-zod'),
  pluginFaker: createAddon('faker', '@relevate/poolster-plugin-faker'),
  pluginMsw: createAddon('msw', '@relevate/poolster-plugin-msw'),
  pluginCypress: createAddon('cypress', '@relevate/poolster-plugin-cypress'),
  pluginReactQuery: createAddon('react-query', '@relevate/poolster-plugin-react-query'),
  pluginVueQuery: createAddon('vue-query', '@relevate/poolster-plugin-vue-query'),
  pluginSwr: createAddon('swr', '@relevate/poolster-plugin-swr'),
  inputGraphql: () => ({ kind: 'native-input', name: '@relevate/poolster-input-graphql', format: 'graphql', provider: 'graphql.apollo' }),
  inputAsyncApi: () => ({ kind: 'native-input', name: '@relevate/poolster-input-asyncapi', format: 'asyncapi', provider: 'asyncapi.roas' }),
  inputArazzo: () => ({ kind: 'native-input', name: '@relevate/poolster-input-arazzo', format: 'arazzo', provider: 'arazzo.roas' }),
  inputProtobuf: () => ({ kind: 'native-input', name: '@relevate/poolster-input-protobuf', format: 'protobuf', provider: 'protobuf.protox' }),
  inputCapnProto: () => ({ kind: 'native-input', name: '@relevate/poolster-input-capnproto', format: 'capnproto', provider: 'capnproto.capnp' }),
};
