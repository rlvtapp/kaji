import { appendFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const REPOSITORY = /^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?\/[A-Za-z0-9_.-]+$/;
function requireThat(condition, message) { if (!condition) throw new Error(message); }
function httpsUrl(value, label) {
  let url; try { url = new URL(value); } catch { throw new Error(`${label} must be an HTTPS URL`); }
  requireThat(url.protocol === 'https:' && !url.username && !url.password && !url.hash, `${label} must be an HTTPS URL without credentials or fragment`);
  return url;
}
async function json(response, description) {
  requireThat(response.ok, `${description} failed (HTTP ${response.status})`);
  const declared = Number(response.headers.get('content-length'));
  requireThat(!Number.isFinite(declared) || declared <= 32768, `${description} response is too large`);
  let size = 0; const chunks = [];
  for await (const chunk of response.body ?? []) { size += chunk.length; requireThat(size <= 32768, `${description} response is too large`); chunks.push(chunk); }
  try { return JSON.parse(Buffer.concat(chunks).toString('utf8')); } catch { throw new Error(`${description} returned invalid JSON`); }
}
export async function requestInstallationToken({ brokerUrl, audience = 'kaji', repositories, oidcUrl, oidcRequestToken, fetch: fetcher = globalThis.fetch }) {
  const broker = httpsUrl(brokerUrl, 'Broker URL');
  requireThat(!broker.search, 'Broker URL must not contain a query');
  requireThat(typeof audience === 'string' && audience.length > 0 && audience.length <= 200, 'Audience is required');
  requireThat(Array.isArray(repositories) && repositories.length > 0 && repositories.length <= 50 && repositories.every(name => typeof name === 'string' && REPOSITORY.test(name)) && new Set(repositories.map(name => name.toLowerCase())).size === repositories.length, 'Repositories must be a nonempty JSON array of distinct owner/repo strings');
  const oidc = httpsUrl(oidcUrl, 'OIDC request URL');
  requireThat(oidc.hostname.endsWith('.actions.githubusercontent.com') && oidc.port === '', 'OIDC request URL must use the official GitHub Actions domain');
  requireThat(typeof oidcRequestToken === 'string' && oidcRequestToken.length > 0 && !/[\r\n]/.test(oidcRequestToken), 'GitHub Actions id-token: write permission is required');
  oidc.searchParams.set('audience', audience);
  const options = { redirect: 'error', signal: AbortSignal.timeout(15000) };
  const identity = await json(await fetcher(oidc, { ...options, headers: { Authorization: `Bearer ${oidcRequestToken}` } }), 'OIDC token request');
  requireThat(identity && typeof identity.value === 'string' && identity.value.length > 0 && identity.value.length <= 16384 && /^[A-Za-z0-9_.-]+$/.test(identity.value), 'OIDC response did not contain a JWT');
  broker.pathname = `${broker.pathname.replace(/\/$/, '')}/token`;
  const result = await json(await fetcher(broker, { ...options, method: 'POST', headers: { Authorization: `Bearer ${identity.value}`, 'Content-Type': 'application/json' }, body: JSON.stringify({ repositories }) }), 'Broker exchange');
  requireThat(result && typeof result.token === 'string' && result.token.length > 0 && result.token.length <= 8192 && !/\s/.test(result.token) && typeof result.expires_at === 'string' && !/[\r\n]/.test(result.expires_at) && Number.isFinite(Date.parse(result.expires_at)), 'Broker did not return a valid installation token');
  requireThat(result.permissions && typeof result.permissions === 'object' && !Array.isArray(result.permissions) && Object.entries(result.permissions).every(([name, permission]) => ['contents', 'pull_requests'].includes(name) && ['read', 'write'].includes(permission)) && ['read', 'write'].includes(result.permissions.contents), 'Broker returned unexpected installation permissions');
  requireThat(Array.isArray(result.repositories) && result.repositories.length === repositories.length && result.repositories.every(name => repositories.some(expected => expected.toLowerCase() === String(name).toLowerCase())) && new Set(result.repositories.map(name => String(name).toLowerCase())).size === repositories.length, 'Broker returned an unexpected repository scope');
  return result;
}
export async function revokeInstallationToken(token, fetcher = globalThis.fetch) {
  requireThat(typeof token === 'string' && token.length > 0 && token.length <= 8192 && !/\s/.test(token), 'An installation token is required for revocation');
  const response = await fetcher('https://api.github.com/installation/token', { method: 'DELETE', redirect: 'error', signal: AbortSignal.timeout(15000), headers: { Authorization: `Bearer ${token}`, Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2026-03-10', 'User-Agent': 'kaji-github-app-broker-client' } });
  requireThat(response.status === 204 || response.status === 401, `Installation token revocation failed (HTTP ${response.status})`);
}
export async function run(environment = process.env, { fetch: fetcher = globalThis.fetch, log = message => console.log(message) } = {}) {
  if (process.argv.includes('--revoke')) { await revokeInstallationToken(environment.KAJI_INSTALLATION_TOKEN, fetcher); return; }
  requireThat(environment.GITHUB_OUTPUT, 'This client must run in a GitHub Actions step with GITHUB_OUTPUT');
  let repositories;
  try { repositories = JSON.parse(environment.KAJI_BROKER_REPOSITORIES ?? ''); } catch { throw new Error('Repositories input must be a JSON array'); }
  const result = await requestInstallationToken({ brokerUrl: environment.KAJI_BROKER_URL, audience: environment.KAJI_BROKER_AUDIENCE ?? 'kaji', repositories, oidcUrl: environment.ACTIONS_ID_TOKEN_REQUEST_URL, oidcRequestToken: environment.ACTIONS_ID_TOKEN_REQUEST_TOKEN, fetch: fetcher });
  const mask = result.token.replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
  log(`::add-mask::${mask}`);
  await appendFile(environment.GITHUB_OUTPUT, `token=${result.token}\nexpires-at=${result.expires_at}\n`, { encoding: 'utf8', mode: 0o600 });
}
if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  run().catch(error => { console.error(error instanceof Error ? error.message : 'Token exchange failed'); process.exitCode = 1; });
}
