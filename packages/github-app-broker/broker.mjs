import { createHash, createPrivateKey, createPublicKey, sign, verify } from 'node:crypto';
import { mkdir, open, readdir, readFile, stat, unlink } from 'node:fs/promises';
import { join } from 'node:path';

export const ISSUER = 'https://token.actions.githubusercontent.com';
export const JWKS_URL = `${ISSUER}/.well-known/jwks`;
export const GITHUB_API = 'https://api.github.com';
const API_VERSION = '2026-03-10';
const REPOSITORY = /^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?\/[A-Za-z0-9_.-]+$/;
const SHA = /^[a-f0-9]{40}$/;
const EVENTS = new Set(['push', 'workflow_dispatch', 'schedule']);
const MAX_JWT = 16384;
const MAX_RESPONSE = 1048576;
const ID = /^[1-9][0-9]*$/;

export class BrokerError extends Error {
  constructor(code, status = 403) { super(code); this.code = code; this.status = status; }
}
function requireThat(condition, code, status = 403) { if (!condition) throw new BrokerError(code, status); }
function object(value) { return value && typeof value === 'object' && !Array.isArray(value); }
function exactKeys(value, allowed, code = 'invalid_configuration') {
  requireThat(object(value) && Object.keys(value).every(key => allowed.includes(key)), code, 400);
}
function identifier(value) { return typeof value === 'string' && ID.test(value) && Number.isSafeInteger(Number(value)); }
function strings(value) { return Array.isArray(value) && value.length > 0 && value.every(item => typeof item === 'string' && item.length > 0) && new Set(value).size === value.length; }
function repository(value) { return typeof value === 'string' && value.length <= 200 && REPOSITORY.test(value) && !value.endsWith('/.') && !value.endsWith('/..'); }

/** Policy is administrator-owned; the caller cannot supply IDs or permissions. */
export function validatePolicy(input) {
  exactKeys(input, ['version', 'audience', 'sources']);
  requireThat(input.version === 1 && typeof input.audience === 'string' && input.audience.length > 0 && input.audience.length <= 200 && Array.isArray(input.sources) && input.sources.length > 0, 'invalid_configuration', 400);
  const policy = structuredClone(input);
  const sourceIds = new Set();
  for (const source of policy.sources) {
    exactKeys(source, ['repository', 'repository_id', 'repository_owner_id', 'subjects', 'workflow_ref', 'workflow_sha', 'job_workflow_ref', 'job_workflow_sha', 'refs', 'events', 'environment', 'runner_environment', 'permissions', 'targets']);
    requireThat(repository(source.repository) && identifier(source.repository_id) && identifier(source.repository_owner_id) && !sourceIds.has(source.repository_id), 'invalid_source_identity', 400);
    sourceIds.add(source.repository_id);
    requireThat(strings(source.subjects) && strings(source.refs) && source.refs.every(ref => /^refs\/heads\/[A-Za-z0-9_./-]+$/.test(ref) && !ref.includes('..')), 'invalid_source_ref_or_subject', 400);
    requireThat(typeof source.workflow_ref === 'string' && source.refs.some(ref => source.workflow_ref.startsWith(`${source.repository}/.github/workflows/`) && source.workflow_ref.endsWith(`@${ref}`)) && !source.workflow_ref.includes('..'), 'invalid_source_workflow', 400);
    requireThat(strings(source.events) && source.events.every(event => EVENTS.has(event)), 'unsafe_source_events', 400);
    for (const claim of ['workflow_sha', 'job_workflow_sha']) requireThat(source[claim] === undefined || (typeof source[claim] === 'string' && SHA.test(source[claim])), 'invalid_workflow_sha', 400);
    if (source.job_workflow_ref !== undefined) requireThat(typeof source.job_workflow_ref === 'string' && /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\/\.github\/workflows\/[A-Za-z0-9_.-]+\.ya?ml@[A-Za-z0-9_./-]+$/.test(source.job_workflow_ref) && !source.job_workflow_ref.includes('..') && source.job_workflow_sha !== undefined, 'reusable_workflow_requires_pinned_sha', 400);
    requireThat(source.job_workflow_sha === undefined || source.job_workflow_ref !== undefined, 'invalid_reusable_workflow', 400);
    requireThat(source.environment === undefined || (typeof source.environment === 'string' && source.environment.length > 0), 'invalid_environment', 400);
    source.runner_environment ??= 'github-hosted';
    requireThat(['github-hosted', 'self-hosted'].includes(source.runner_environment), 'invalid_runner_environment', 400);
    exactKeys(source.permissions, ['contents', 'pull_requests']);
    requireThat(['read', 'write'].includes(source.permissions.contents) && (source.permissions.pull_requests === undefined || ['read', 'write'].includes(source.permissions.pull_requests)), 'unsafe_permissions', 400);
    requireThat(Array.isArray(source.targets) && source.targets.length > 0 && source.targets.length <= 50, 'invalid_targets', 400);
    const names = new Set(); const ids = new Set();
    for (const target of source.targets) {
      exactKeys(target, ['repository', 'repository_id', 'installation_id']);
      requireThat(repository(target.repository) && identifier(target.repository_id) && identifier(target.installation_id) && !names.has(target.repository.toLowerCase()) && !ids.has(target.repository_id), 'invalid_target_identity', 400);
      names.add(target.repository.toLowerCase()); ids.add(target.repository_id);
    }
  }
  return policy;
}

async function responseJson(response) {
  requireThat(response.ok, 'upstream_unavailable', 502);
  const length = Number(response.headers.get('content-length'));
  requireThat(!Number.isFinite(length) || length <= MAX_RESPONSE, 'upstream_response_too_large', 502);
  const chunks = []; let size = 0;
  for await (const chunk of response.body ?? []) {
    size += chunk.length; requireThat(size <= MAX_RESPONSE, 'upstream_response_too_large', 502); chunks.push(chunk);
  }
  try { const result = JSON.parse(Buffer.concat(chunks).toString('utf8')); requireThat(object(result), 'invalid_upstream_response', 502); return result; }
  catch (error) { if (error instanceof BrokerError) throw error; throw new BrokerError('invalid_upstream_response', 502); }
}

/** Atomic, persistent single-use OIDC jti markers; shared by broker restarts. */
export class FileReplayStore {
  constructor(directory, { now = () => Date.now(), limit = 100000 } = {}) { this.directory = directory; this.now = now; this.limit = limit; this.lastCleanup = 0; }
  async consume(key, expiresAt) {
    await mkdir(this.directory, { recursive: true, mode: 0o700 });
    if (this.now() - this.lastCleanup > 60000 || this.lastCleanup === 0) {
      for (const file of await readdir(this.directory)) {
        if (!/^[a-f0-9]{64}$/.test(file)) continue;
        const path = join(this.directory, file);
        try { const expiration = await readFile(path, 'utf8'); if ((/^[0-9]+$/.test(expiration) && Number(expiration) <= this.now()) || (expiration === '' && (await stat(path)).mtimeMs < this.now() - 700000)) await unlink(path); } catch (error) { if (error.code !== 'ENOENT') throw error; }
      }
      this.lastCleanup = this.now();
    }
    requireThat((await readdir(this.directory)).length < this.limit, 'replay_store_capacity', 503);
    const file = join(this.directory, createHash('sha256').update(key).digest('hex'));
    let handle;
    try { handle = await open(file, 'wx', 0o600); }
    catch (error) { if (error.code === 'EEXIST') throw new BrokerError('token_already_used'); throw error; }
    try { await handle.writeFile(String(expiresAt)); } finally { await handle.close(); }
  }
}

/** Useful for tests and embedded single-process instances only. */
export class MemoryReplayStore {
  constructor({ now = () => Date.now(), limit = 10000 } = {}) { this.now = now; this.limit = limit; this.entries = new Map(); }
  async consume(key, expiresAt) {
    for (const [entry, expiration] of this.entries) if (expiration <= this.now()) this.entries.delete(entry);
    requireThat(!this.entries.has(key), 'token_already_used');
    requireThat(this.entries.size < this.limit, 'replay_store_capacity', 503);
    this.entries.set(key, expiresAt);
  }
}

function segment(value) {
  requireThat(typeof value === 'string' && /^[A-Za-z0-9_-]+$/.test(value), 'invalid_oidc_token', 401);
  const bytes = Buffer.from(value, 'base64url');
  requireThat(bytes.toString('base64url') === value, 'invalid_oidc_token', 401);
  return bytes;
}
function segmentJson(value) {
  try { const result = JSON.parse(segment(value).toString('utf8')); requireThat(object(result), 'invalid_oidc_token', 401); return result; }
  catch (error) { if (error instanceof BrokerError) throw error; throw new BrokerError('invalid_oidc_token', 401); }
}

export function appJwt(appId, privateKey, now = Date.now()) {
  const seconds = Math.floor(now / 1000);
  const header = Buffer.from(JSON.stringify({ alg: 'RS256', typ: 'JWT' })).toString('base64url');
  const payload = Buffer.from(JSON.stringify({ iat: seconds - 60, exp: seconds + 540, iss: String(appId) })).toString('base64url');
  const unsigned = `${header}.${payload}`;
  return `${unsigned}.${sign('RSA-SHA256', Buffer.from(unsigned), privateKey).toString('base64url')}`;
}

export function createBroker({ policy: input, appId, privateKey, replayStore, fetch: fetcher = globalThis.fetch, now = () => Date.now() }) {
  const policy = validatePolicy(input);
  requireThat(identifier(String(appId)), 'invalid_app_id', 400);
  const key = createPrivateKey(privateKey);
  requireThat(key.asymmetricKeyType === 'rsa' && key.asymmetricKeyDetails.modulusLength >= 2048, 'invalid_app_private_key', 400);
  requireThat(replayStore && typeof replayStore.consume === 'function', 'persistent_replay_store_required', 400);
  let keys = []; let fetchedAt = -Infinity; let refresh;
  async function request(url, options = {}) {
    try { return await fetcher(url, { ...options, redirect: 'error', signal: AbortSignal.timeout(10000) }); }
    catch { throw new BrokerError('upstream_unavailable', 502); }
  }
  async function refreshKeys() {
    if (!refresh) refresh = (async () => {
      const jwks = await responseJson(await request(JWKS_URL));
      requireThat(Array.isArray(jwks.keys) && jwks.keys.length > 0 && jwks.keys.length <= 100, 'invalid_jwks', 502);
      keys = jwks.keys; fetchedAt = now();
    })().finally(() => { refresh = undefined; });
    await refresh;
  }
  async function verifyOidc(token) {
    requireThat(typeof token === 'string' && token.length <= MAX_JWT, 'invalid_oidc_token', 401);
    const parts = token.split('.'); requireThat(parts.length === 3, 'invalid_oidc_token', 401);
    const header = segmentJson(parts[0]); const claims = segmentJson(parts[1]);
    requireThat(header.alg === 'RS256' && header.typ === 'JWT' && typeof header.kid === 'string' && header.kid.length > 0 && header.kid.length <= 200 && header.crit === undefined && header.b64 === undefined && header.jku === undefined && header.jwk === undefined && header.x5u === undefined, 'invalid_oidc_header', 401);
    if (now() - fetchedAt >= 300000) await refreshKeys();
    let matches = keys.filter(key => key.kid === header.kid);
    if (matches.length === 0 && now() - fetchedAt >= 60000) { await refreshKeys(); matches = keys.filter(key => key.kid === header.kid); }
    requireThat(matches.length === 1, 'unknown_oidc_key', 401);
    const jwk = matches[0];
    requireThat(jwk.kty === 'RSA' && (jwk.alg === undefined || jwk.alg === 'RS256') && (jwk.use === undefined || jwk.use === 'sig') && (jwk.key_ops === undefined || (Array.isArray(jwk.key_ops) && jwk.key_ops.includes('verify'))) && typeof jwk.n === 'string' && jwk.n.length <= 1400 && typeof jwk.e === 'string', 'invalid_jwks_key', 502);
    let publicKey;
    try { publicKey = createPublicKey({ key: jwk, format: 'jwk' }); }
    catch { throw new BrokerError('invalid_jwks_key', 502); }
    requireThat(publicKey.asymmetricKeyDetails.modulusLength >= 2048 && publicKey.asymmetricKeyDetails.modulusLength <= 8192, 'invalid_jwks_key', 502);
    requireThat(verify('RSA-SHA256', Buffer.from(`${parts[0]}.${parts[1]}`), publicKey, segment(parts[2])), 'invalid_oidc_signature', 401);
    const seconds = Math.floor(now() / 1000);
    requireThat(claims.iss === ISSUER && claims.aud === policy.audience, 'invalid_oidc_issuer_or_audience', 401);
    requireThat(['iat', 'nbf', 'exp'].every(claim => Number.isSafeInteger(claims[claim])) && claims.exp > seconds && claims.iat <= seconds + 30 && claims.nbf <= seconds + 30 && claims.iat >= seconds - 600 && claims.exp > claims.iat && claims.exp - claims.iat <= 600 && claims.nbf <= claims.exp, 'invalid_oidc_time', 401);
    requireThat(typeof claims.jti === 'string' && claims.jti.length > 0 && claims.jti.length <= 200 && typeof claims.sub === 'string', 'invalid_oidc_identity', 401);
    return claims;
  }
  function authorize(claims, repositories) {
    requireThat(Array.isArray(repositories) && repositories.length > 0 && repositories.length <= 50 && repositories.every(repository), 'invalid_repositories', 400);
    const names = repositories.map(name => name.toLowerCase());
    requireThat(new Set(names).size === names.length, 'duplicate_repositories', 400);
    const source = policy.sources.find(source => source.repository_id === claims.repository_id);
    requireThat(source && source.repository === claims.repository && source.repository_owner_id === claims.repository_owner_id && source.repository.split('/')[0] === claims.repository_owner, 'source_not_trusted');
    requireThat(source.subjects.includes(claims.sub) && source.refs.includes(claims.ref) && claims.ref_type === 'branch' && (claims.ref_protected === 'true' || claims.ref_protected === true), 'source_ref_not_trusted');
    requireThat(EVENTS.has(claims.event_name) && source.events.includes(claims.event_name) && !claims.head_ref && !claims.base_ref, 'source_event_not_trusted');
    requireThat(source.workflow_ref === claims.workflow_ref && typeof claims.workflow_sha === 'string' && SHA.test(claims.workflow_sha) && (source.workflow_sha === undefined || source.workflow_sha === claims.workflow_sha) && (source.job_workflow_ref === undefined ? !claims.job_workflow_ref && !claims.job_workflow_sha : source.job_workflow_ref === claims.job_workflow_ref && source.job_workflow_sha === claims.job_workflow_sha), 'source_workflow_not_trusted');
    requireThat(source.environment === undefined || source.environment === claims.environment, 'source_environment_not_trusted');
    requireThat(source.runner_environment === claims.runner_environment, 'source_runner_not_trusted');
    requireThat(typeof claims.sha === 'string' && SHA.test(claims.sha) && identifier(claims.run_id) && identifier(claims.run_attempt), 'invalid_run_identity');
    const targets = names.map(name => source.targets.find(target => target.repository.toLowerCase() === name));
    requireThat(targets.every(Boolean), 'target_not_allowed');
    requireThat(new Set(targets.map(target => target.installation_id)).size === 1, 'targets_require_separate_installations', 400);
    return { source, targets };
  }
  async function github(path, token, options = {}) {
    return request(`${GITHUB_API}${path}`, { ...options, headers: { Accept: 'application/vnd.github+json', Authorization: `Bearer ${token}`, 'X-GitHub-Api-Version': API_VERSION, 'User-Agent': 'kaji-github-app-broker', 'Content-Type': 'application/json' } });
  }
  async function exchange(oidcToken, repositories) {
    const claims = await verifyOidc(oidcToken);
    const { source, targets } = authorize(claims, repositories);
    await replayStore.consume(`${ISSUER}:${claims.jti}`, claims.exp * 1000);
    const jwt = appJwt(appId, key, now());
    const result = await responseJson(await github(`/app/installations/${targets[0].installation_id}/access_tokens`, jwt, { method: 'POST', body: JSON.stringify({ repository_ids: targets.map(target => Number(target.repository_id)), permissions: source.permissions }) }));
    requireThat(typeof result.token === 'string' && result.token.length > 0 && result.token.length <= 8192 && !/\s/.test(result.token), 'invalid_installation_token', 502);
    try {
      const expiration = Date.parse(result.expires_at);
      requireThat(Number.isFinite(expiration) && expiration > now() && expiration <= now() + 3630000, 'invalid_installation_expiration', 502);
      requireThat(object(result.permissions), 'invalid_installation_permissions', 502);
      for (const [name, permission] of Object.entries(result.permissions)) requireThat(name === 'metadata' ? permission === 'read' : source.permissions[name] === permission, 'installation_permission_escalation', 502);
      for (const [name, permission] of Object.entries(source.permissions)) requireThat(result.permissions[name] === permission, 'installation_permissions_missing', 502);
      // Independently check the returned token's effective scope before handing it out.
      const scope = await responseJson(await github('/installation/repositories?per_page=100', result.token));
      requireThat(scope.total_count === targets.length && Array.isArray(scope.repositories) && scope.repositories.length === targets.length, 'installation_repository_escalation', 502);
      requireThat(scope.repositories.every(repo => targets.some(target => Number(target.repository_id) === repo.id && target.repository.toLowerCase() === String(repo.full_name).toLowerCase())) && new Set(scope.repositories.map(repo => repo.id)).size === targets.length, 'installation_repository_mismatch', 502);
      return { token: result.token, expires_at: result.expires_at, repositories: targets.map(target => target.repository), permissions: source.permissions };
    } catch (error) {
      try { await github('/installation/token', result.token, { method: 'DELETE' }); } catch { /* Never return a token whose effective scope could not be checked. */ }
      throw error;
    }
  }
  return { exchange, verifyOidc };
}
