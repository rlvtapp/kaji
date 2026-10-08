import assert from 'node:assert/strict';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const packageRoot = process.argv[2];
if (!packageRoot) throw new Error('Pass the generated TypeScript package directory');

const load = (module) => import(pathToFileURL(path.resolve(packageRoot, 'dist', module)).href);
const validation = await load('validation.js');
const queries = await load('queries.js');

assert.deepEqual(validation.WidgetSchema.parse({ id: 'w1', name: 'Widget' }), { id: 'w1', name: 'Widget' });
assert.equal(validation.WidgetSchema.safeParse({ id: 123, name: 'Widget' }).success, false);
assert.deepEqual(validation.CreateWidgetSchema.parse({ name: 'New' }), { name: 'New' });
assert.equal(validation.CreateWidgetSchema.safeParse({}).success, false);
const listResponse = validation.ListWidgetsResponseSchemas['200']['application/json'];
assert.equal(listResponse.safeParse([{ id: 'w1', name: 'Widget' }]).success, true);
assert.equal(listResponse.safeParse([{ id: 123, name: 'Widget' }]).success, false);

const listKey = queries.listWidgetsQueryKey({ query: { limit: 2 } }, 'tenant-a');
assert.equal(listKey[0], 'listWidgets');
assert.equal(listKey[1], 'tenant-a');
assert.deepEqual(listKey[2].query, { limit: 2 });
const getKey = queries.getWidgetQueryKey({ path: { widgetId: 'w1' } }, 'tenant-a');
assert.equal(getKey[0], 'getWidget');
assert.deepEqual(getKey[2].path, { widgetId: 'w1' });
assert.deepEqual(queries.listWidgetsQueryOptions({}, {}, 'tenant-a').queryKey, queries.listWidgetsQueryKey({}, 'tenant-a'));

console.log('Generated Zod validators and React Query keys behave as expected.');
