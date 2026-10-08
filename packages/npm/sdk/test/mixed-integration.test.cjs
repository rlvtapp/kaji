'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const test = require('node:test');

const { defineContract, generate, providerHandle, requireContract } = require('../index.cjs');
const bundle = require('../../plugins-all/index.cjs');
const { artifacts, config, temporary } = require('../test-support/helpers.cjs');

function file(result, name) {
  const found = result.files.find((entry) => entry.path === name);
  assert.ok(found, `missing generated file ${name}`);
  return found.contents;
}

test('Rust plugins selected from JS emit meaningful, connected SDK output', async (t) => {
  const compiled = await artifacts(t);
  const dir = await temporary(t);
  const extensions = [
    ['zod', bundle.pluginZod], ['faker', bundle.pluginFaker],
    ['msw', bundle.pluginMsw], ['cypress', bundle.pluginCypress],
    ['react-query', bundle.pluginReactQuery], ['vue-query', bundle.pluginVueQuery],
    ['swr', bundle.pluginSwr],
  ];
  const result = await generate(config({ artifacts: compiled }, path.join(dir, 'all'), [
    bundle.pluginTypeScript({ path: 'web', name: '@example/widgets' }),
    ...extensions.map(([name, factory]) => factory({ target: 'web', output: `extensions/${name}` })),
  ]), { write: false });

  const zod = file(result, 'web/extensions/zod.ts');
  assert.match(zod, /WidgetSchema = z\.object\(\{ "id": z\.string\(\), "name": z\.string\(\)/);
  assert.match(zod, /ListWidgetsResponseSchemas/);
  assert.match(zod, /"application\/json": z\.array\(z\.lazy\(\(\) => WidgetSchema\)\)/);

  const faker = file(result, 'web/extensions/faker.ts');
  assert.match(faker, /export function createWidget\(__depth = 0\): Widget/);
  assert.match(faker, /export const seedPoolsterFixtures/);
  assert.match(faker, /"Widget":/);

  const msw = file(result, 'web/extensions/msw.ts');
  assert.match(msw, /http\.all\("\/widgets\/:widgetId"/);
  assert.match(msw, /if \(request\.method !== "GET"\)/);
  assert.match(msw, /if \(request\.method !== "POST"\)/);

  const cypress = file(result, 'web/extensions/cypress.ts');
  assert.match(cypress, /cy\.request\(\{ method: "GET", url: baseUrl \+ "\/widgets"/);

  for (const name of ['react-query', 'vue-query', 'swr']) {
    const code = file(result, `web/extensions/${name}.ts`);
    assert.match(code, /listWidgetsQueryKey/);
    assert.match(code, /getWidgetQueryKey/);
    assert.match(code, /from "\.\.\/clients\/widgets\/listWidgets\.js"/);
  }

  const manifest = JSON.parse(file(result, 'web/package.json'));
  assert.equal(manifest.name, '@example/widgets');
  assert.equal(manifest.dependencies.zod, '^4.0.0');
  assert.equal(manifest.dependencies['@faker-js/faker'], '^9.0.0');
  assert.equal(manifest.dependencies.msw, '^2.0.0');
  assert.equal(manifest.devDependencies.cypress, '^15.0.0');
  assert.equal(manifest.peerDependencies['@tanstack/react-query'], '^5.0.0');
  assert.equal(manifest.peerDependencies['@tanstack/vue-query'], '^5.0.0');
  assert.equal(manifest.peerDependencies.swr, '^2.0.0');

  const generatedPaths = new Set(result.files.map((entry) => entry.path));
  for (const [name] of extensions) {
    const generated = result.files.filter((entry) => entry.path.startsWith(`web/extensions/${name}`));
    assert.ok(generated.length, `${name} did not emit a file`);
    for (const entry of generated) {
      for (const [, relative] of entry.contents.matchAll(/from ["'](\.[^"']+)["']/g)) {
        const importPath = path.posix.normalize(path.posix.join(path.posix.dirname(entry.path), relative.replace(/\.js$/, '.ts')));
        assert.ok(generatedPaths.has(importPath), `${entry.path} imports missing ${importPath}`);
      }
    }
  }
});

test('JS transforms, Rust plugins, JS contracts, and regeneration compose safely', async (t) => {
  const compiled = await artifacts(t);
  const dir = await temporary(t);
  const output = path.join(dir, 'mixed');
  const Inspected = defineContract('inspected-native-output');
  const filter = {
    name: 'public-only',
    transformApi(api) {
      return { ...api, operations: api.operations.filter((operation) => !operation.path.startsWith('/internal/')) };
    },
  };
  const inspector = {
    name: 'inspect-native',
    requires: ['@relevate/poolster-plugin-zod', '@relevate/poolster-plugin-react-query'],
    provides: [Inspected],
    generate(ctx) {
      const validation = ctx.readFile('web/validation.ts');
      const queries = ctx.readFile('web/queries.ts');
      assert.match(validation, /WidgetSchema/);
      assert.match(queries, /listWidgetsQueryKey/);
      assert.doesNotMatch(queries, /internalHealth/);
      ctx.replaceFile('web/validation.ts', `${validation}\n// reviewed by JS\n`);
      ctx.publish(Inspected, { operations: ctx.api.operations.length, files: 2 });
    },
  };
  const report = {
    name: 'report', phase: 'post',
    requires: [requireContract(Inspected, { from: providerHandle(inspector, Inspected) })],
    generate(ctx) {
      assert.match(ctx.readFile('web/validation.ts'), /reviewed by JS/);
      ctx.emitFile({ path: 'mix.json', contents: `${JSON.stringify(ctx.inputs.get(Inspected))}\n` });
    },
  };
  const mixed = config({ artifacts: compiled }, output, [
    report,
    bundle.pluginZod({ target: 'web', output: 'validation' }),
    inspector,
    filter,
    bundle.pluginReactQuery({ target: 'web', output: 'queries' }),
    bundle.pluginTypeScript({ path: 'web' }),
  ]);

  const first = await generate(mixed);
  assert.deepEqual(JSON.parse(await fs.readFile(path.join(output, 'mix.json'), 'utf8')), { operations: 3, files: 2 });
  assert.match(await fs.readFile(path.join(output, 'web/validation.ts'), 'utf8'), /reviewed by JS/);
  assert.ok(first.changes.added.includes('web/validation.ts'));
  const clean = await generate(mixed);
  assert.deepEqual(clean.changes, { added: [], modified: [], removed: [] });

  await fs.writeFile(path.join(output, 'web/validation.ts'), 'author edit\n');
  await assert.rejects(generate(mixed), /edited|modified|overwrite/i);
  assert.equal(await fs.readFile(path.join(output, 'web/validation.ts'), 'utf8'), 'author edit\n');
  await fs.writeFile(path.join(output, 'web/validation.ts'), file(first, 'web/validation.ts'));

  const reduced = await generate(config({ artifacts: compiled }, output, [bundle.pluginTypeScript({ path: 'web' })]));
  assert.ok(reduced.changes.removed.includes('web/validation.ts'));
  assert.ok(reduced.changes.removed.includes('web/queries.ts'));
  assert.ok(reduced.changes.removed.includes('mix.json'));
  await assert.rejects(fs.stat(path.join(output, 'mix.json')), { code: 'ENOENT' });
});
