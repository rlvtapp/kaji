import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const version = JSON.parse(fs.readFileSync(path.join(root, 'packages/npm/sdk/package.json'), 'utf8')).version;
const license = fs.readFileSync(path.join(root, 'LICENSE'), 'utf8');
const check = process.argv.includes('--check');
const missing = [];
const generated = new Map();

const plugins = [
  ['typescript', 'TypeScript'],
  ['rust', 'Rust'],
  ['go', 'Go'],
  ['python', 'Python'],
  ['php', 'Php'],
  ['java', 'Java'],
  ['csharp', 'CSharp'],
  ['elixir', 'Elixir'],
  ['ruby', 'Ruby'],
  ['swift', 'Swift'],
];
const auxiliaries = [
  ['zod', 'Zod'],
  ['faker', 'Faker'],
  ['msw', 'Msw'],
  ['cypress', 'Cypress'],
  ['react-query', 'ReactQuery'],
  ['vue-query', 'VueQuery'],
  ['swr', 'Swr'],
];
const inputs = [
  ['graphql', 'Graphql', 'graphql.apollo'],
  ['asyncapi', 'AsyncApi', 'asyncapi.roas'],
  ['arazzo', 'Arazzo', 'arazzo.roas'],
  ['protobuf', 'Protobuf', 'protobuf.protox'],
  ['capnproto', 'CapnProto', 'capnproto.capnp'],
];

function write(directory, file, contents) {
  generated.set(`${directory}/${file}`, contents);
  const destination = path.join(here, directory, file);
  if (check) {
    if (!fs.existsSync(destination) || fs.readFileSync(destination, 'utf8') !== contents) missing.push(destination);
    return;
  }
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.writeFileSync(destination, contents);
}

function writeNode(file, contents) {
  const destination = path.join(root, 'packages/npm/sdk', file);
  if (check) {
    if (!fs.existsSync(destination) || fs.readFileSync(destination, 'utf8') !== contents) missing.push(destination);
    return;
  }
  fs.writeFileSync(destination, contents);
}

function manifest(name) {
  return `${JSON.stringify({
    name,
    version,
    description: `Poolster plugin exports for ${name.endsWith('-plugins') ? 'bundled inputs and outputs' : name.split('-').at(-1)}`,
    license: 'MIT',
    repository: { type: 'git', url: 'git+https://github.com/rlvtapp/kaji.git' },
    main: 'index.cjs',
    types: 'index.d.ts',
    exports: { '.': { types: './index.d.ts', import: './index.mjs', require: './index.cjs' } },
    files: ['index.cjs', 'index.mjs', 'index.d.ts', 'README.md', 'LICENSE'],
    engines: { node: '>=18' },
    peerDependencies: { '@relevate/poolster': version },
    publishConfig: { access: 'public' },
  }, null, 2)}\n`;
}

function optionsType(language) {
  if (language === 'typescript') return "Omit<SdkPackageOptions, 'language' | 'jobs'>";
  if (language === 'go') return "Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw'>";
  return "Omit<SdkPackageOptions, 'language' | 'transport' | 'clientName' | 'raw' | 'jobs'>";
}

for (const [language, exportSuffix] of plugins) {
  const name = `@relevate/poolster-plugin-${language}`;
  const exportName = `plugin${exportSuffix}`;
  const descriptor = `  return { kind: 'native-sdk', name: '${name}', package: { ...options, language: '${language}', path: options.path ?? '${language}' } };`;
  write(`plugins/${language}`, 'package.json', manifest(name));
  write(`plugins/${language}`, 'index.cjs', `'use strict';\n\nfunction ${exportName}(options = {}) {\n  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('${exportName} options must be an object');\n${descriptor}\n}\n\nmodule.exports = { ${exportName} };\n`);
  write(`plugins/${language}`, 'index.mjs', `import api from './index.cjs';\n\nexport const ${exportName} = api.${exportName};\n`);
  write(`plugins/${language}`, 'index.d.ts', `import type { NativePlugin, SdkPackageOptions } from '@relevate/poolster';\n\nexport type ${exportSuffix}PluginOptions = ${optionsType(language)};\nexport function ${exportName}(options?: ${exportSuffix}PluginOptions): NativePlugin;\n`);
  write(`plugins/${language}`, 'README.md', `# ${name}\n\nSelect Poolster's ${language} SDK renderer in a JavaScript config. Install this package\nwith \`@relevate/poolster\`, then add \`${exportName}()\` to the config's\n\`plugins\` array. Nothing is registered automatically.\n\n\`\`\`js\nimport { defineConfig } from '@relevate/poolster'\nimport { ${exportName} } from '${name}'\n\nexport default defineConfig({\n  input: './openapi.yaml',\n  output: { path: './generated' },\n  name: 'Example',\n  version: '1.0.0',\n  plugins: [${exportName}({ path: '${language}' })],\n})\n\`\`\`\n`);
  write(`plugins/${language}`, 'LICENSE', license);
}

for (const [id, exportSuffix] of auxiliaries) {
  const name = `@relevate/poolster-plugin-${id}`;
  const exportName = `plugin${exportSuffix}`;
  const descriptor = `  return { kind: 'native-addon', name: '${name}', plugin: '${id}', target: options.target ?? 'typescript', ...(options.output ? { output: options.output } : {}) };`;
  write(`plugins/${id}`, 'package.json', manifest(name));
  write(`plugins/${id}`, 'index.cjs', `'use strict';\n\nfunction ${exportName}(options = {}) {\n  if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError('${exportName} options must be an object');\n  for (const key of Object.keys(options)) if (key !== 'target' && key !== 'output') throw new TypeError('unknown ${exportName} option ' + key);\n  if (options.target != null && (typeof options.target !== 'string' || !options.target)) throw new TypeError('target must be a nonempty string');\n  if (options.output != null && (typeof options.output !== 'string' || !options.output)) throw new TypeError('output must be a nonempty string');\n${descriptor}\n}\n\nmodule.exports = { ${exportName} };\n`);
  write(`plugins/${id}`, 'index.mjs', `import api from './index.cjs';\n\nexport const ${exportName} = api.${exportName};\n`);
  write(`plugins/${id}`, 'index.d.ts', `import type { NativeAddon } from '@relevate/poolster';\n\nexport interface ${exportSuffix}PluginOptions { target?: string; output?: string }\nexport function ${exportName}(options?: ${exportSuffix}PluginOptions): NativeAddon;\n`);
  write(`plugins/${id}`, 'README.md', `# ${name}\n\nSelect Poolster's compiled Rust ${id} plugin from a JavaScript config. Install\nwith \`@relevate/poolster\` and \`@relevate/poolster-plugin-typescript\`, then\nlist both factories in \`plugins\`. Nothing is registered automatically.\n\n\`\`\`js\nimport { defineConfig } from '@relevate/poolster'\nimport { pluginTypeScript } from '@relevate/poolster-plugin-typescript'\nimport { ${exportName} } from '${name}'\n\nexport default defineConfig({\n  input: './openapi.yaml',\n  output: './generated',\n  name: 'Example',\n  version: '1.0.0',\n  plugins: [pluginTypeScript(), ${exportName}({ target: 'typescript' })],\n})\n\`\`\`\n\nThe \`target\` names the TypeScript SDK package path; use it when the package\npath differs from the default or multiple TypeScript packages are selected.\n`);
  write(`plugins/${id}`, 'LICENSE', license);
}

for (const [format, suffix, provider] of inputs) {
  const name = `@relevate/poolster-input-${format}`;
  const exportName = `input${suffix}`;
  const folder = `inputs/${format}`;
  write(folder, 'package.json', manifest(name));
  write(folder, 'index.cjs', `'use strict';\n\nfunction ${exportName}() {\n  return { kind: 'native-input', name: '${name}', format: '${format}', provider: '${provider}' };\n}\n\nmodule.exports = { ${exportName} };\n`);
  write(folder, 'index.mjs', `import api from './index.cjs';\n\nexport const ${exportName} = api.${exportName};\n`);
  write(folder, 'index.d.ts', `import type { NativeInputPlugin } from '@relevate/poolster';\n\nexport function ${exportName}(): NativeInputPlugin;\n`);
  write(folder, 'README.md', `# ${name}\n\nSelect Poolster's Rust ${format} input provider from JavaScript. Install with \`@relevate/poolster\` and add \`${exportName}()\` to \`input.plugin\` in your config. Native format contracts can be inspected and consumed by JavaScript output plugins through \`ctx.input\`. HTTP SDK renderers require an HTTP contract.\n`);
  write(folder, 'LICENSE', license);
}

const bundleName = '@relevate/poolster-plugins';
const factoryEntries = plugins.map(([language, suffix]) => `  plugin${suffix}: create('${language}', '@relevate/poolster-plugin-${language}'),`).join('\n');
const addonEntries = auxiliaries.map(([id, suffix]) => `  plugin${suffix}: createAddon('${id}', '@relevate/poolster-plugin-${id}'),`).join('\n');
const inputEntries = inputs.map(([format, suffix, provider]) => `  input${suffix}: () => ({ kind: 'native-input', name: '@relevate/poolster-input-${format}', format: '${format}', provider: '${provider}' }),`).join('\n');
const esmEntries = [...plugins, ...auxiliaries].map(([, suffix]) => `export const plugin${suffix} = api.plugin${suffix};`).concat(inputs.map(([, suffix]) => `export const input${suffix} = api.input${suffix};`)).join('\n');
const typeEntries = plugins.map(([language, suffix]) => `export type ${suffix}PluginOptions = ${optionsType(language)};\nexport function plugin${suffix}(options?: ${suffix}PluginOptions): NativePlugin;`).join('\n\n');
const addonTypes = auxiliaries.map(([, suffix]) => `export interface ${suffix}PluginOptions { target?: string; output?: string }\nexport function plugin${suffix}(options?: ${suffix}PluginOptions): NativeAddon;`).join('\n\n');
const inputTypes = inputs.map(([, suffix]) => `export function input${suffix}(): NativeInputPlugin;`).join('\n');
write('plugins-all', 'package.json', manifest(bundleName));
write('plugins-all', 'index.cjs', `'use strict';\n\nfunction create(language, name) {\n  return (options = {}) => {\n    if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError(\`\${name} options must be an object\`);\n    return { kind: 'native-sdk', name, package: { ...options, language, path: options.path ?? language } };\n  };\n}\n\nfunction createAddon(plugin, name) {\n  return (options = {}) => {\n    if (!options || typeof options !== 'object' || Array.isArray(options)) throw new TypeError(\`\${name} options must be an object\`);\n    for (const key of Object.keys(options)) if (key !== 'target' && key !== 'output') throw new TypeError(\`unknown \${name} option \${key}\`);\n    if (options.target != null && (typeof options.target !== 'string' || !options.target)) throw new TypeError('target must be a nonempty string');\n    if (options.output != null && (typeof options.output !== 'string' || !options.output)) throw new TypeError('output must be a nonempty string');\n    return { kind: 'native-addon', name, plugin, target: options.target ?? 'typescript', ...(options.output ? { output: options.output } : {}) };\n  };\n}\n\nmodule.exports = {\n${factoryEntries}\n${addonEntries}\n${inputEntries}\n};\n`);
write('plugins-all', 'index.mjs', `import api from './index.cjs';\n\n${esmEntries}\n`);
write('plugins-all', 'index.d.ts', `import type { NativePlugin, NativeAddon, NativeInputPlugin, SdkPackageOptions } from '@relevate/poolster';\n\n${typeEntries}\n\n${addonTypes}\n\n${inputTypes}\n`);
write('plugins-all', 'README.md', `# ${bundleName}\n\nA bundle of Poolster's native SDK and compiled Rust auxiliary plugin factories.\nInstall with \`@relevate/poolster\` and add only the plugins you want to\n\`poolster.config.mjs\`. Installation does not register any renderer.\n\nLanguage exports: ${plugins.map(([, suffix]) => `\`plugin${suffix}\``).join(', ')}.\n\nAuxiliary exports: ${auxiliaries.map(([, suffix]) => `\`plugin${suffix}\``).join(', ')}.\n\nInput exports: inputGraphql, inputAsyncApi, inputArazzo, inputProtobuf, inputCapnProto.\n\nFor a smaller install, use individual \`@relevate/poolster-plugin-<name>\` packages\ninstead. These factories select implementations compiled into Poolster's addon.\n`);
write('plugins-all', 'LICENSE', license);

for (const extension of ['cjs', 'mjs', 'd.ts']) {
  const contents = generated.get(`plugins-all/index.${extension}`);
  writeNode(`plugins.${extension}`, extension === 'mjs'
    ? contents.replace("import api from './index.cjs'", "import api from './plugins.cjs'")
    : contents);
}

if (missing.length) {
  console.error(`Node plugin packages are out of sync:\n${missing.join('\n')}`);
  process.exitCode = 1;
} else if (!check) {
  console.log(`Generated ${plugins.length} language packages, ${auxiliaries.length} Rust auxiliary packages, ${inputs.length} input packages, and the bundle at version ${version}`);
}
