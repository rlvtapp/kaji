import test from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync, sign, verify } from 'node:crypto';
import { mkdtemp, readFile, rm, utimes, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { BrokerError, createBroker, FileReplayStore, MemoryReplayStore, validatePolicy, ISSUER, JWKS_URL, GITHUB_API } from './broker.mjs';
import { createBrokerServer } from './server.mjs';
import { requestInstallationToken, revokeInstallationToken, run } from './client.mjs';

const oidcKeys = generateKeyPairSync('rsa', { modulusLength: 2048 });
const appKeys = generateKeyPairSync('rsa', { modulusLength: 2048 });
const attackerKeys = generateKeyPairSync('rsa', { modulusLength: 2048 });
const appPem = appKeys.privateKey.export({ type: 'pkcs8', format: 'pem' });
const jwk = { ...oidcKeys.publicKey.export({ format: 'jwk' }), kid: 'github-fixture', alg: 'RS256', use: 'sig' };
const epoch = Date.UTC(2026, 9, 7, 12, 0, 0);
const policy = {
  version: 1, audience: 'kaji', sources: [{
    repository: 'example/api', repository_id: '123456', repository_owner_id: '123',
    subjects: ['repo:example@123/api@123456:ref:refs/heads/main'],
    workflow_ref: 'example/api/.github/workflows/kaji-sdk.yml@refs/heads/main',
    refs: ['refs/heads/main'], events: ['push', 'workflow_dispatch'], runner_environment: 'github-hosted',
    permissions: { contents: 'write', pull_requests: 'write' }, targets: [
      { repository: 'example/typescript-sdk', repository_id: '234567', installation_id: '345678' },
      { repository: 'example/python-sdk', repository_id: '234568', installation_id: '345678' },
    ],
  }],
};
function claims(overrides = {}) {
  const time = Math.floor(epoch / 1000);
  return { iss: ISSUER, aud: 'kaji', sub: policy.sources[0].subjects[0], iat: time - 10, nbf: time - 10, exp: time + 290, jti: 'single-use-token-1', repository: 'example/api', repository_id: '123456', repository_owner: 'example', repository_owner_id: '123', workflow_ref: policy.sources[0].workflow_ref, workflow_sha: 'a'.repeat(40), ref: 'refs/heads/main', ref_type: 'branch', ref_protected: 'true', event_name: 'push', head_ref: '', base_ref: '', sha: 'a'.repeat(40), run_id: '1111', run_attempt: '1', runner_environment: 'github-hosted', ...overrides };
}
function jwt(payload = claims(), { header = {}, key = oidcKeys.privateKey } = {}) {
  const head = Buffer.from(JSON.stringify({ alg: 'RS256', typ: 'JWT', kid: 'github-fixture', ...header })).toString('base64url');
  const body = Buffer.from(JSON.stringify(payload)).toString('base64url');
  return `${head}.${body}.${sign('RSA-SHA256', Buffer.from(`${head}.${body}`), key).toString('base64url')}`;
}
function fixture({ inputPolicy = policy, permissions, repositories, expiration, fetchOverride, now = () => epoch, jwks = { keys: [jwk] } } = {}) {
  const calls = [];
  const fetcher = async (url, options) => {
    url = String(url); calls.push({ url, options });
    assert.equal(options.redirect, 'error');
    if (fetchOverride) { const result = await fetchOverride(url, options); if (result) return result; }
    if (url === JWKS_URL) return Response.json(jwks);
    if (url === `${GITHUB_API}/app/installations/345678/access_tokens`) {
      const unsignedAppJwt = options.headers.Authorization.slice(7).split('.');
      assert.equal(unsignedAppJwt.length, 3);
      assert.ok(verify('RSA-SHA256', Buffer.from(unsignedAppJwt.slice(0, 2).join('.')), appKeys.publicKey, Buffer.from(unsignedAppJwt[2], 'base64url')));
      const appClaims = JSON.parse(Buffer.from(unsignedAppJwt[1], 'base64url'));
      assert.equal(appClaims.iss, '555'); assert.ok(appClaims.exp - appClaims.iat <= 600);
      assert.deepEqual(JSON.parse(options.body).permissions, inputPolicy.sources[0].permissions);
      return Response.json({ token: 'ghs_555.JWT.fixture', expires_at: expiration ?? new Date(now() + 3600000).toISOString(), permissions: permissions ?? { contents: 'write', pull_requests: 'write', metadata: 'read' } }, { status: 201 });
    }
    if (url === `${GITHUB_API}/installation/repositories?per_page=100`) {
      assert.equal(options.headers.Authorization, 'Bearer ghs_555.JWT.fixture');
      const requested = JSON.parse(calls.find(call => call.url.includes('/access_tokens')).options.body).repository_ids;
      const selected = repositories ?? policy.sources[0].targets.filter(target => requested.includes(Number(target.repository_id))).map(target => ({ id: Number(target.repository_id), full_name: target.repository }));
      return Response.json({ total_count: selected.length, repositories: selected });
    }
    if (url === `${GITHUB_API}/installation/token`) { assert.equal(options.method, 'DELETE'); return new Response(null, { status: 204 }); }
    throw new Error('Unexpected outbound URL');
  };
  const broker = createBroker({ policy: inputPolicy, appId: '555', privateKey: appPem, replayStore: new MemoryReplayStore({ now }), now, fetch: fetcher });
  return { broker, calls };
}
async function denied(fixtureOptions, token, repositories, code) {
  const { broker, calls } = fixture(fixtureOptions);
  await assert.rejects(broker.exchange(token, repositories), error => error instanceof BrokerError && error.code === code);
  assert.equal(calls.filter(call => call.url.includes('/access_tokens')).length, 0);
}

test('signed OIDC mints a separately verified, repository-scoped installation token', async () => {
  const { broker, calls } = fixture();
  const result = await broker.exchange(jwt(), ['example/typescript-sdk', 'example/python-sdk']);
  assert.equal(result.token, 'ghs_555.JWT.fixture');
  assert.deepEqual(JSON.parse(calls[1].options.body), { repository_ids: [234567, 234568], permissions: { contents: 'write', pull_requests: 'write' } });
  assert.equal(calls.length, 3);
});

test('signature validation rejects forged tokens and JOSE algorithm/key URL tricks', async () => {
  for (const [token, code] of [
    [jwt(claims(), { key: attackerKeys.privateKey }), 'invalid_oidc_signature'],
    [jwt(claims(), { header: { alg: 'none' } }), 'invalid_oidc_header'],
    [jwt(claims(), { header: { alg: 'HS256' } }), 'invalid_oidc_header'],
    [jwt(claims(), { header: { typ: 'at+jwt' } }), 'invalid_oidc_header'],
    [jwt(claims(), { header: { kid: 'attacker' } }), 'unknown_oidc_key'],
    [jwt(claims(), { header: { jku: 'https://attacker.example/keys' } }), 'invalid_oidc_header'],
    [jwt(claims(), { header: { crit: ['anything'] } }), 'invalid_oidc_header'],
    ['header.payload', 'invalid_oidc_token'],
    ['x'.repeat(20000), 'invalid_oidc_token'],
  ]) await denied({}, token, ['example/typescript-sdk'], code);
});

test('issuer audience expiration issue time and required claims fail closed', async () => {
  const seconds = Math.floor(epoch / 1000);
  for (const override of [{ iss: 'https://attacker.example' }, { aud: 'different' }, { aud: ['kaji'] }, { exp: seconds }, { exp: seconds + 900 }, { iat: seconds + 60 }, { nbf: seconds + 60 }, { iat: seconds - 1000 }, { exp: '9999999999' }, { nbf: undefined }, { jti: '' }]) {
    const code = Object.hasOwn(override, 'iss') || Object.hasOwn(override, 'aud') ? 'invalid_oidc_issuer_or_audience' : Object.hasOwn(override, 'jti') ? 'invalid_oidc_identity' : 'invalid_oidc_time';
    await denied({}, jwt(claims(override)), ['example/typescript-sdk'], code);
  }
});

test('exact source and workflow trust rejects fork and PR context even on protected main', async () => {
  for (const [override, code] of [
    [{ repository_id: '999' }, 'source_not_trusted'], [{ repository: 'attacker/api' }, 'source_not_trusted'], [{ repository_owner_id: '999' }, 'source_not_trusted'],
    [{ sub: 'repo:example/api:pull_request' }, 'source_ref_not_trusted'], [{ ref: 'refs/heads/unprotected' }, 'source_ref_not_trusted'], [{ ref_protected: 'false' }, 'source_ref_not_trusted'], [{ ref_type: 'tag' }, 'source_ref_not_trusted'],
    [{ event_name: 'pull_request' }, 'source_event_not_trusted'], [{ event_name: 'pull_request_target' }, 'source_event_not_trusted'], [{ head_ref: 'fork' }, 'source_event_not_trusted'], [{ event_name: 'dynamic' }, 'source_event_not_trusted'],
    [{ workflow_ref: 'example/api/.github/workflows/other.yml@refs/heads/main' }, 'source_workflow_not_trusted'], [{ job_workflow_ref: 'attacker/reusable/.github/workflows/steal.yml@main', job_workflow_sha: 'c'.repeat(40) }, 'source_workflow_not_trusted'], [{ runner_environment: 'self-hosted' }, 'source_runner_not_trusted'], [{ sha: 'not-a-sha' }, 'invalid_run_identity'],
  ]) await denied({}, jwt(claims(override)), ['example/typescript-sdk'], code);
});

test('configured environment and immutable reusable workflow SHA are enforced', async () => {
  const constrained = structuredClone(policy);
  Object.assign(constrained.sources[0], { environment: 'sdk-release', job_workflow_ref: 'example/automation/.github/workflows/sdk.yml@v1', job_workflow_sha: 'b'.repeat(40) });
  await denied({ inputPolicy: constrained }, jwt(claims({ environment: 'other', job_workflow_ref: constrained.sources[0].job_workflow_ref, job_workflow_sha: 'b'.repeat(40) })), ['example/typescript-sdk'], 'source_environment_not_trusted');
  await denied({ inputPolicy: constrained }, jwt(claims({ environment: 'sdk-release', job_workflow_ref: constrained.sources[0].job_workflow_ref, job_workflow_sha: 'c'.repeat(40) })), ['example/typescript-sdk'], 'source_workflow_not_trusted');
  const { broker } = fixture({ inputPolicy: constrained });
  await broker.exchange(jwt(claims({ environment: 'sdk-release', job_workflow_ref: constrained.sources[0].job_workflow_ref, job_workflow_sha: 'b'.repeat(40) })), ['example/typescript-sdk']);
});

test('caller cannot request arbitrary repositories or mix installations', async () => {
  await denied({}, jwt(), ['example/unconfigured'], 'target_not_allowed');
  await denied({}, jwt(), ['example/typescript-sdk', 'EXAMPLE/typescript-sdk'], 'duplicate_repositories');
  await denied({}, jwt(), [], 'invalid_repositories');
  const mixed = structuredClone(policy); mixed.sources[0].targets[1].installation_id = '4444';
  await denied({ inputPolicy: mixed }, jwt(), ['example/typescript-sdk', 'example/python-sdk'], 'targets_require_separate_installations');
});

test('overscoped or malformed GitHub responses are revoked and never returned', async () => {
  for (const [options, code] of [
    [{ permissions: { contents: 'write', pull_requests: 'write', workflows: 'write' } }, 'installation_permission_escalation'],
    [{ permissions: { contents: 'write' } }, 'installation_permissions_missing'],
    [{ repositories: [{ id: 234567, full_name: 'example/typescript-sdk' }, { id: 999, full_name: 'example/other' }] }, 'installation_repository_escalation'],
    [{ repositories: [{ id: 234567, full_name: 'attacker/transferred-sdk' }] }, 'installation_repository_mismatch'],
    [{ expiration: new Date(epoch + 7200000).toISOString() }, 'invalid_installation_expiration'],
  ]) {
    const { broker, calls } = fixture(options);
    await assert.rejects(broker.exchange(jwt(), ['example/typescript-sdk']), error => error.code === code);
    assert.equal(calls.filter(call => call.options.method === 'DELETE').length, 1);
  }
});

test('configuration rejects broad permission grants and ambiguous source IDs', () => {
  const invalid = structuredClone(policy); invalid.sources[0].permissions.workflows = 'write';
  assert.throws(() => validatePolicy(invalid), error => error.code === 'invalid_configuration');
  const unpinned = structuredClone(policy); unpinned.sources[0].job_workflow_ref = 'example/reusable/.github/workflows/sdk.yml@main';
  assert.throws(() => validatePolicy(unpinned), error => error.code === 'reusable_workflow_requires_pinned_sha');
  const duplicate = structuredClone(policy); duplicate.sources.push(duplicate.sources[0]);
  assert.throws(() => validatePolicy(duplicate), error => error.code === 'invalid_source_identity');
});

test('single-use identifiers reject concurrent replays and JWKS cache bounds refresh', async () => {
  const { broker, calls } = fixture();
  const outcomes = await Promise.allSettled([broker.exchange(jwt(), ['example/typescript-sdk']), broker.exchange(jwt(), ['example/typescript-sdk'])]);
  assert.equal(outcomes.filter(outcome => outcome.status === 'fulfilled').length, 1);
  assert.equal(outcomes.find(outcome => outcome.status === 'rejected').reason.code, 'token_already_used');
  assert.equal(calls.filter(call => call.url === JWKS_URL).length, 1);
  await assert.rejects(broker.verifyOidc(jwt(claims(), { header: { kid: 'attacker' } })), error => error.code === 'unknown_oidc_key');
  assert.equal(calls.filter(call => call.url === JWKS_URL).length, 1);
});

test('persistent replay store survives a restart and does not remove empty in-progress markers', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'kaji-broker-replay-'));
  try {
    const first = new FileReplayStore(directory, { now: () => epoch });
    await first.consume('same-jti', epoch + 300000);
    const restarted = new FileReplayStore(directory, { now: () => epoch });
    await assert.rejects(restarted.consume('same-jti', epoch + 300000), error => error.code === 'token_already_used');
    await writeFile(join(directory, 'a'.repeat(64)), '');
    await utimes(join(directory, 'a'.repeat(64)), epoch / 1000, epoch / 1000);
    await new FileReplayStore(directory, { now: () => epoch }).consume('different-jti', epoch + 300000);
    assert.equal(await readFile(join(directory, 'a'.repeat(64)), 'utf8'), '');
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('HTTP endpoint limits input shape and never records bearer or installation secrets', async () => {
  const audit = [];
  const server = createBrokerServer({ exchange: async () => ({ token: 'secret-installation-token', expires_at: new Date(epoch + 3600000).toISOString(), repositories: ['example/typescript-sdk'], permissions: { contents: 'write' } }) }, { audit: event => audit.push(event) });
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  const endpoint = `http://127.0.0.1:${server.address().port}/token`;
  try {
    const response = await fetch(endpoint, { method: 'POST', headers: { Authorization: 'Bearer secret.oidc.token', 'Content-Type': 'application/json' }, body: JSON.stringify({ repositories: ['example/typescript-sdk'] }) });
    assert.equal(response.status, 200); assert.equal(response.headers.get('cache-control'), 'no-store');
    assert.equal((await response.json()).token, 'secret-installation-token');
    const denied = await fetch(endpoint, { method: 'POST', headers: { Authorization: 'Bearer secret.oidc.token', 'Content-Type': 'application/json' }, body: JSON.stringify({ repositories: ['example/typescript-sdk'], permissions: { actions: 'write' } }) });
    assert.equal(denied.status, 400);
    assert.ok(!JSON.stringify(audit).includes('secret'));
  } finally { server.closeAllConnections(); await new Promise(done => server.close(done)); }
});

test('composite client requests official OIDC, sends JSON scope and masks file outputs', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'kaji-broker-action-')); const output = join(directory, 'output'); const logs = []; const calls = [];
  const clientFetch = async (url, options) => {
    calls.push({ url: String(url), options }); assert.equal(options.redirect, 'error');
    if (String(url).startsWith('https://pipelines.actions.githubusercontent.com/')) { assert.equal(new URL(url).searchParams.get('audience'), 'kaji'); return Response.json({ value: jwt() }); }
    assert.equal(String(url), 'https://broker.example/kaji/token'); assert.deepEqual(JSON.parse(options.body), { repositories: ['example/typescript-sdk'] });
    return Response.json({ token: 'ghs_555.JWT.fixture', expires_at: new Date(epoch + 3600000).toISOString(), repositories: ['example/typescript-sdk'], permissions: { contents: 'write', pull_requests: 'write' } });
  };
  try {
    await run({ GITHUB_OUTPUT: output, KAJI_BROKER_URL: 'https://broker.example/kaji', KAJI_BROKER_REPOSITORIES: '["example/typescript-sdk"]', ACTIONS_ID_TOKEN_REQUEST_URL: 'https://pipelines.actions.githubusercontent.com/oidc?api-version=2', ACTIONS_ID_TOKEN_REQUEST_TOKEN: 'request-secret' }, { fetch: clientFetch, log: message => logs.push(message) });
    assert.equal(logs[0], '::add-mask::ghs_555.JWT.fixture');
    assert.ok((await readFile(output, 'utf8')).includes('token=ghs_555.JWT.fixture\n')); assert.equal(calls.length, 2);
    await assert.rejects(requestInstallationToken({ brokerUrl: 'http://broker.example', repositories: ['example/typescript-sdk'] }), /HTTPS/);
    await assert.rejects(requestInstallationToken({ brokerUrl: 'https://broker.example', repositories: ['example/typescript-sdk'], oidcUrl: 'https://attacker.example/oidc', oidcRequestToken: 'request-secret' }), /official GitHub/);
    await revokeInstallationToken('ghs_fixture', async (url, options) => { assert.equal(url, `${GITHUB_API}/installation/token`); assert.equal(options.method, 'DELETE'); return new Response(null, { status: 204 }); });
  } finally { await rm(directory, { recursive: true, force: true }); }
});
