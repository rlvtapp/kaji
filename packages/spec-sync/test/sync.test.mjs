import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import os from 'node:os';
import { mkdtemp, writeFile, rm, symlink, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { syncSpec, validateReferences, safePath } from '../sync.mjs';
const source = 'openapi: 3.1.0\ncomponents:\n  schemas:\n    Note:\n      $ref: "#/components/schemas/Other"\n';
const sourceSha = 'a'.repeat(40);
async function fixture(t) {
  const workspace = await mkdtemp(path.join(os.tmpdir(), 'poolster-spec-'));
  t.after(() => rm(workspace, { recursive: true, force: true }));
  await writeFile(path.join(workspace, 'openapi.yaml'), source);
  return { workspace, repository: 'acme/sdk', sourceRepository: 'acme/api', sourceSha, sourcePath: 'openapi.yaml', targetPath: 'specs/openapi.yaml', token: 'secret-never-log' };
}
function mock({ branch = true, spec = null, provenance = null, pr = false, fail = null, ahead = 1 } = {}) {
  const calls = [];
  const fetch = async (url, options) => {
    const request = { url, method: options.method, body: options.body ? JSON.parse(options.body) : undefined, headers: options.headers }; calls.push(request);
    const route = new URL(url).pathname.replace('/repos/acme/sdk', '');
    let status = 200, value;
    if (fail && route.includes(fail)) status = 409;
    else if (route === '/git/ref/heads/main') value = { object: { sha: 'base' } };
    else if (route.startsWith('/git/ref/heads/')) { if (branch) value = { object: { sha: 'review-parent' } }; else status = 404; }
    else if (route.startsWith('/git/commits/') && options.method === 'GET') value = { tree: { sha: 'original-tree' } };
    else if (route.startsWith('/contents/')) { const content = route.endsWith('.poolster/spec-source.json') ? provenance : spec; if (content === null) status = 404; else value = { type: 'file', encoding: 'base64', content: Buffer.from(content).toString('base64') }; }
    else if (route === '/git/blobs') value = { sha: `blob-${calls.length}` };
    else if (route === '/git/trees') value = { sha: 'new-tree' };
    else if (route === '/git/commits') value = { sha: 'new-commit' };
    else if (route.startsWith('/git/refs')) value = {};
    else if (route.startsWith('/compare/')) value = { ahead_by: ahead };
    else if (route === '/pulls' && options.method === 'GET') value = pr ? [{ number: 12, html_url: 'https://github.com/acme/sdk/pull/12' }] : [];
    else if (route.startsWith('/pulls')) value = { number: 12, html_url: 'https://github.com/acme/sdk/pull/12' };
    else throw new Error(`Unexpected request ${options.method} ${route}`);
    return { status, ok: status >= 200 && status < 300, json: async () => value };
  };
  return { calls, fetch };
}
test('existing review branch preserves its parent/tree and uses a nonforce ref update', async t => {
  const input = await fixture(t), api = mock({ pr: true });
  const result = await syncSpec(input, api);
  assert.equal(result.changed, true); assert.equal(result.pullRequestUrl, 'https://github.com/acme/sdk/pull/12');
  const commit = api.calls.find(call => call.method === 'POST' && call.url.endsWith('/git/commits'));
  assert.deepEqual(commit.body.parents, ['review-parent']);
  const tree = api.calls.find(call => call.url.endsWith('/git/trees'));
  assert.equal(tree.body.base_tree, 'original-tree'); assert.deepEqual(tree.body.tree.map(item => item.path), ['specs/openapi.yaml', '.poolster/spec-source.json']);
  assert.deepEqual(api.calls.find(call => call.method === 'PATCH' && call.url.includes('/git/refs')).body, { sha: 'new-commit', force: false });
  assert.equal(api.calls.filter(call => call.method === 'POST' && call.url.endsWith('/pulls')).length, 0);
  const recorded = JSON.parse(Buffer.from(api.calls.filter(call => call.url.endsWith('/git/blobs'))[1].body.content, 'base64'));
  assert.equal(recorded.source_repository, 'acme/api'); assert.equal(recorded.source_sha, sourceSha); assert.equal(recorded.spec_sha256, createHash('sha256').update(source).digest('hex'));
  assert.ok(api.calls.every(call => call.headers.Authorization === 'Bearer secret-never-log'));
});
test('new branch starts at base and opens a PR without force pushing', async t => {
  const input = await fixture(t), api = mock({ branch: false });
  await syncSpec(input, api);
  assert.deepEqual(api.calls.find(call => call.url.endsWith('/git/commits') && call.method === 'POST').body.parents, ['base']);
  assert.deepEqual(api.calls.find(call => call.url.endsWith('/git/refs')).body, { ref: 'refs/heads/codex/poolster-spec-sync', sha: 'new-commit' });
  const pr = api.calls.find(call => call.url.endsWith('/pulls') && call.method === 'POST');
  assert.equal(pr.body.base, 'main'); assert.equal(pr.body.head, 'codex/poolster-spec-sync');
});
test('identical source and provenance do not create another commit', async t => {
  const input = await fixture(t);
  const provenance = JSON.stringify({ schema_version: 1, source_repository: input.sourceRepository, source_sha: sourceSha, source_path: input.sourcePath, spec_sha256: createHash('sha256').update(source).digest('hex') }, null, 2) + '\n';
  const api = mock({ spec: source, provenance, pr: true });
  assert.equal((await syncSpec(input, api)).changed, false);
  assert.ok(api.calls.every(call => !call.url.includes('/git/') || call.method === 'GET'));
});
test('concurrent branch updates fail without creating a PR or retrying with force', async t => {
  const input = await fixture(t), api = mock({ fail: '/git/refs' });
  await assert.rejects(syncSpec(input, api), /409/);
  assert.ok(!api.calls.some(call => call.url.endsWith('/pulls')));
  assert.ok(!api.calls.some(call => call.body?.force === true));
});
test('source symlinks, path escapes and workflow targets are rejected before GitHub access', async t => {
  const input = await fixture(t); await symlink(path.join(input.workspace, 'openapi.yaml'), path.join(input.workspace, 'linked.yaml'));
  for (const changed of [{sourcePath:'linked.yaml'}, {sourcePath:'../outside.yaml'}, {targetPath:'.github/workflows/release.yml'}, {targetPath:'.git/config'}, {targetPath:'.poolster/spec-source.json'}, {branch:'main'}]) {
    const api = mock(); await assert.rejects(syncSpec({...input, ...changed}, api)); assert.equal(api.calls.length, 0);
  }
  for (const value of ['a/../b', '/tmp/a', 'a\\b', 'a//b', '.GITHUB/spec.yaml']) assert.throws(() => safePath(value));
});
test('JSON/YAML relative external refs fail explicitly; internal and absolute URLs pass', () => {
  for (const spec of ['{"$ref":"./schemas.yaml#/Thing"}', '$ref: ../schemas.yaml', '"$ref": \'models.yaml#/Thing\'', '$ref: |\n  models.yaml', '$ref: *alias', '{"nested":{"$ref":"models.json"}}', '"\\u0024ref": "models.yaml"']) assert.throws(() => validateReferences(spec), /relative|unsupported/i);
  for (const spec of ['{"$ref":"#/components/schemas/A"}', "$ref: '#/components/schemas/A'", '$ref: https://example.com/openapi.yaml#/A', '# $ref: ignored.yaml\nopenapi: 3.1.0']) assert.doesNotThrow(() => validateReferences(spec));
});
test('network failure output cannot expose the token', async t => {
  const input = await fixture(t);
  await assert.rejects(syncSpec(input, { fetch: async () => {throw new Error(input.token);} }), error => !error.message.includes(input.token));
});
test('action uses env inputs with editable fixed helper commands', async () => {
  const action = await readFile(new URL('../action.yml', import.meta.url), 'utf8');
  assert.ok(action.includes('POOLSTER_SPEC_TOKEN: ${{ inputs.token }}'));
  assert.ok(action.includes('run: node "$POOLSTER_ACTION_PATH/sync.mjs"'));
  assert.ok(!/^\s*run:.*\$\{\{/m.test(action));
});

test('existing provenance protects manual edits and other source connections', async t => {
  const input = await fixture(t);
  for (const recorded of [
    {schema_version:1,source_sha:sourceSha,source_repository: input.sourceRepository, source_path: input.sourcePath, spec_sha256: 'b'.repeat(64)},
    {schema_version:1,source_sha:sourceSha,source_repository: 'another/api', source_path: input.sourcePath, spec_sha256: createHash('sha256').update(source).digest('hex')},
  ]) {
    const api = mock({spec: source, provenance: JSON.stringify(recorded)});
    await assert.rejects(syncSpec(input,api), /manual edits|another source/);
    assert.ok(api.calls.every(call => call.method === 'GET'));
  }
  await assert.rejects(syncSpec(input,mock({spec:source})), /no provenance/);
});
test('unchanged bytes retain the last spec-changing SHA and a merged branch does not open an empty PR', async t => {
  const input = await fixture(t);
  const provenance = JSON.stringify({schema_version:1,source_repository:input.sourceRepository,source_path:input.sourcePath,source_sha:'b'.repeat(40),spec_sha256:createHash('sha256').update(source).digest('hex')})+'\n';
  const api=mock({spec:source,provenance,ahead:0});
  const result=await syncSpec(input,api);
  assert.equal(result.changed,false);assert.equal(result.pullRequestUrl,'');
  assert.ok(api.calls.every(call=>call.method==='GET'));
});

test('large existing destination files use immutable Git blobs when contents API omits bytes', async t => {
  const input=await fixture(t);
  const provenance=JSON.stringify({schema_version:1,source_sha:sourceSha,source_repository:input.sourceRepository,source_path:input.sourcePath,spec_sha256:createHash('sha256').update(source).digest('hex')})+'\n';
  const api=mock({spec:source,provenance,pr:true});
  const original=api.fetch;
  api.fetch=async(url,options)=>{
    if(url.includes('/contents/specs/openapi.yaml'))return {status:200,ok:true,json:async()=>({type:'file',encoding:'none',sha:'c'.repeat(40)})};
    if(url.endsWith('/git/blobs/'+'c'.repeat(40)))return {status:200,ok:true,json:async()=>({encoding:'base64',content:Buffer.from(source).toString('base64')})};
    return original(url,options);
  };
  assert.equal((await syncSpec(input,api)).changed,false);
});
