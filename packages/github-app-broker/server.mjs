import { createServer } from 'node:http';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { BrokerError, createBroker, FileReplayStore } from './broker.mjs';

const MAX_BODY = 16384;
export function createBrokerServer(broker, { now = () => Date.now(), audit = event => console.error(JSON.stringify(event)), rateLimit = 30, maxConcurrent = 20 } = {}) {
  const rates = new Map(); let concurrent = 0;
  const server = createServer({ maxHeaderSize: 24576, requestTimeout: 15000, headersTimeout: 10000 }, async (request, response) => {
    const requestId = randomUUID();
    response.setHeader('Cache-Control', 'no-store'); response.setHeader('Content-Type', 'application/json'); response.setHeader('X-Content-Type-Options', 'nosniff');
    const send = (status, body) => { response.writeHead(status); response.end(JSON.stringify(body)); };
    if (request.method === 'GET' && request.url === '/healthz') { send(200, { status: 'ok' }); return; }
    if (request.method !== 'POST' || request.url !== '/token') { send(404, { error: 'not_found', request_id: requestId }); return; }
    // Trust the connection peer only. Public ingress must also rate-limit;
    // arbitrary X-Forwarded-For values must not bypass the local limit.
    const peer = request.socket.remoteAddress ?? 'unknown';
    for (const [key, entry] of rates) if (entry.until <= now()) rates.delete(key);
    const entry = rates.get(peer) ?? { count: 0, until: now() + 60000 };
    entry.count++; rates.set(peer, entry);
    if (rates.size > 10000 || entry.count > rateLimit || concurrent >= maxConcurrent) { send(429, { error: 'rate_limited', request_id: requestId }); request.resume(); return; }
    concurrent++;
    try {
      const authHeaders = request.rawHeaders.filter((_, index) => index % 2 === 0).filter(header => header.toLowerCase() === 'authorization');
      if (authHeaders.length !== 1 || !/^Bearer [A-Za-z0-9_.-]+$/.test(request.headers.authorization ?? '')) throw new BrokerError('missing_oidc_token', 401);
      if ((request.headers['content-type'] ?? '').split(';')[0].trim().toLowerCase() !== 'application/json') throw new BrokerError('invalid_content_type', 400);
      const chunks = []; let size = 0;
      for await (const chunk of request) { size += chunk.length; if (size > MAX_BODY) throw new BrokerError('request_too_large', 413); chunks.push(chunk); }
      let body;
      try { body = JSON.parse(Buffer.concat(chunks).toString('utf8')); } catch { throw new BrokerError('invalid_request_json', 400); }
      if (!body || typeof body !== 'object' || Array.isArray(body) || Object.keys(body).length !== 1 || !Object.hasOwn(body, 'repositories')) throw new BrokerError('invalid_request', 400);
      const result = await broker.exchange(request.headers.authorization.slice(7), body.repositories);
      audit({ event: 'installation_token_issued', request_id: requestId, repositories: result.repositories, permissions: result.permissions, expires_at: result.expires_at });
      send(200, result);
    } catch (error) {
      const code = error instanceof BrokerError ? error.code : 'internal_error';
      audit({ event: 'installation_token_denied', request_id: requestId, code });
      if (!response.headersSent) send(error instanceof BrokerError ? error.status : 500, { error: code, request_id: requestId });
      request.resume();
    } finally { concurrent--; }
  });
  server.keepAliveTimeout = 5000;
  return server;
}

export async function start(environment = process.env) {
  const configPath = environment.POOLSTER_BROKER_POLICY;
  if (!configPath) throw new Error('Set POOLSTER_BROKER_POLICY to an administrator-owned policy JSON file');
  const policy = JSON.parse(await readFile(configPath, 'utf8'));
  const privateKey = environment.GITHUB_APP_PRIVATE_KEY ?? (environment.GITHUB_APP_PRIVATE_KEY_FILE ? await readFile(environment.GITHUB_APP_PRIVATE_KEY_FILE, 'utf8') : undefined);
  if (!privateKey || !environment.GITHUB_APP_ID) throw new Error('Set GITHUB_APP_ID and GITHUB_APP_PRIVATE_KEY or GITHUB_APP_PRIVATE_KEY_FILE');
  const replayDirectory = environment.POOLSTER_BROKER_REPLAY_DIRECTORY;
  if (!replayDirectory) throw new Error('Set POOLSTER_BROKER_REPLAY_DIRECTORY to a private persistent replay-marker directory');
  const broker = createBroker({ policy, appId: environment.GITHUB_APP_ID, privateKey, replayStore: new FileReplayStore(resolve(replayDirectory)) });
  const server = createBrokerServer(broker);
  const port = Number(environment.PORT ?? 8787);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid PORT');
  const host = environment.HOST ?? '127.0.0.1';
  await new Promise((done, reject) => { server.once('error', reject); server.listen(port, host, done); });
  console.error(JSON.stringify({ event: 'broker_listening', host, port }));
  return server;
}
if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  start().catch(() => { console.error('Broker startup failed: check policy, private key, app ID and persistent replay directory'); process.exitCode = 1; });
}
