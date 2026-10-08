import path from 'node:path';
import { readFile, realpath, lstat, appendFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const PROVENANCE = '.poolster/spec-source.json';
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
function text(value, label) {
  if (typeof value !== 'string' || !value || /[\x00-\x1f\x7f]/.test(value)) throw new Error(`Invalid ${label}`);
  return value;
}
export function safePath(value, label = 'path') {
  text(value, label);
  const parts = value.split('/');
  if (value.includes('\\') || parts.some(part => !part || part === '.' || part === '..') || path.isAbsolute(value)) throw new Error(`Unsafe ${label}`);
  if (parts.some(part => ['.github', '.git'].includes(part.toLowerCase()))) throw new Error(`${label} cannot address .github or .git`);
  return value;
}
function branchName(value) {
  text(value, 'branch');
  if (!/^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(value) || value.includes('..') || value.includes('//') || value.endsWith('/') || value.endsWith('.') || value.split('/').some(part => part.startsWith('.') || part.endsWith('.lock'))) throw new Error('Invalid branch name');
  return value;
}
function reference(value) {
  if (typeof value !== 'string' || !value || /[\x00-\x1f]/.test(value)) throw new Error('OpenAPI $ref must be a literal string');
  if (value.startsWith('#') || /^[A-Za-z][A-Za-z0-9+.-]*:/.test(value)) return;
  throw new Error(`External relative $ref is unsupported; bundle referenced files before spec sync`);
}
export function validateReferences(source) {
  if (/^\s*[\[{]/.test(source)) {
    let document;
    try { document = JSON.parse(source); } catch { throw new Error('Invalid JSON specification'); }
    const visit = value => {
      if (!value || typeof value !== 'object') return;
      for (const [key, item] of Object.entries(value)) { if (key === '$ref') reference(item); else visit(item); }
    };
    visit(document); return;
  }
  // Conservative YAML scalar scan. Unsupported aliases, tags and block scalar
  // references fail instead of copying a spec whose dependency cannot be checked.
  const clean = source.split('\n').filter(line => !/^\s*#/.test(line)).join('\n');
  if (/(^|[\s{,])[&*][A-Za-z0-9_-]+(?=\s|:|$)/m.test(clean)) throw new Error('YAML aliases/anchors are unsupported; bundle to JSON before spec sync');
  const entries = /(?:^|[\s,{])("(?:[^"\\]|\\.)*"|'(?:[^']|'')*'|\$ref)\s*:\s*("(?:[^"\\]|\\.)*"|'(?:[^']|'')*'|[^\s,}\]]*)/gm;
  for (const match of clean.matchAll(entries)) {
    const decode = raw => raw.startsWith('"') ? JSON.parse(raw) : raw.startsWith("'") ? raw.slice(1, -1).replaceAll("''", "'") : raw;
    let key; try { key = decode(match[1]); } catch { throw new Error('Unsupported YAML key escape syntax'); }
    if (key !== '$ref') continue;
    const raw = match[2];
    if (!raw || /^[&*!>|]/.test(raw)) throw new Error('YAML $ref aliases, tags and multiline scalars are unsupported; bundle to JSON first');
    let value; try { value = decode(raw); } catch { throw new Error('Unsupported YAML $ref escape syntax; bundle to JSON first'); }
    reference(value);
  }
}
async function loadSource(workspace, relative) {
  safePath(relative, 'source-path');
  const root = await realpath(workspace);
  let current = root;
  for (const part of relative.split('/')) {
    current = path.join(current, part);
    if ((await lstat(current)).isSymbolicLink()) throw new Error('Source path cannot contain symlinks');
  }
  const actual = await realpath(current);
  if (!actual.startsWith(`${root}${path.sep}`) || !(await lstat(actual)).isFile()) throw new Error('Source must be a file inside the checkout');
  const bytes = await readFile(actual);
  if (bytes.length > 10 * 1024 * 1024 || !bytes.length) throw new Error('Specification must contain between 1 byte and 10 MiB');
  const source = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  validateReferences(source);
  return bytes;
}
export async function syncSpec(input, dependencies = {}) {
  const repository = text(input.repository, 'repository');
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) throw new Error('repository must be owner/name');
  const sourceRepository = text(input.sourceRepository, 'source repository');
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(sourceRepository)) throw new Error('Source repository must be owner/name');
  const sourceSha = text(input.sourceSha, 'source SHA');
  if (!/^[a-f0-9]{40}$/i.test(sourceSha)) throw new Error('Source SHA must be a full Git commit SHA');
  const branch = branchName(input.branch ?? 'poolster/spec-sync');
  const base = branchName(input.base ?? 'main');
  if (branch === base) throw new Error('Review branch must differ from the base branch');
  const target = safePath(input.targetPath, 'target-path');
  if (target.split('/').some(part => part.toLowerCase() === '.poolster') || !/\.(json|ya?ml)$/i.test(target)) throw new Error('Target must be a JSON/YAML specification file, not provenance');
  const token = text(input.token, 'token');
  const bytes = await loadSource(input.workspace, input.sourcePath);
  let provenance = Buffer.from(`${JSON.stringify({ schema_version: 1, source_repository: sourceRepository, source_sha: sourceSha, source_path: input.sourcePath, spec_sha256: sha256(bytes) }, null, 2)}\n`);
  const fetcher = dependencies.fetch ?? fetch;
  const prefix = `/repos/${repository}`;
  async function api(method, route, body, allowMissing = false) {
    let response;
    try { response = await fetcher(`https://api.github.com${prefix}${route}`, {
      method, headers: { Authorization: `Bearer ${token}`, Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'poolster-spec-sync', ...(body ? { 'Content-Type': 'application/json' } : {}) },
      ...(body ? { body: JSON.stringify(body) } : {}), redirect: 'error', signal: AbortSignal.timeout(30000),
    }); } catch { throw new Error("GitHub request failed before receiving a response"); }
    if (allowMissing && response.status === 404) return null;
    if (!response.ok) throw new Error(`GitHub ${method} request failed (${response.status}); retry after resolving branch changes or permissions`);
    return response.json();
  }
  const refPath = name => name.split('/').map(encodeURIComponent).join('/');
  const baseRef = await api('GET', `/git/ref/heads/${refPath(base)}`);
  let branchRef = await api('GET', `/git/ref/heads/${refPath(branch)}`, undefined, true);
  const parent = branchRef?.object.sha ?? baseRef.object.sha;
  const commit = await api('GET', `/git/commits/${encodeURIComponent(parent)}`);
  const existing = async file => {
    const value = await api('GET', `/contents/${file.split('/').map(encodeURIComponent).join('/')}?ref=${encodeURIComponent(parent)}`, undefined, true);
    if (!value) return null;
    if (value.type !== 'file') throw new Error('Destination spec must be a regular file');
    let content = value;
    // GitHub omits inline content above 1 MiB; use the immutable blob instead.
    if (content.encoding === 'none' && /^[a-f0-9]{40}$/i.test(content.sha ?? '')) content = await api('GET', `/git/blobs/${content.sha}`);
    if (content.encoding !== 'base64' || typeof content.content !== 'string') throw new Error('Destination file does not expose supported Git blob content');
    return Buffer.from(content.content.replaceAll('\n', ''), 'base64');
  };
  const oldSpec = await existing(target);
  const oldProvenance = await existing(PROVENANCE);
  let effectiveSourceSha = sourceSha;
  if (oldProvenance) {
    let recorded;
    try { recorded = JSON.parse(oldProvenance.toString('utf8')); } catch { throw new Error('Existing spec provenance is invalid; preserve branch and reconcile manually'); }
    if (recorded.schema_version !== 1 || !/^[a-f0-9]{40}$/i.test(recorded.source_sha ?? '') || !/^[a-f0-9]{64}$/.test(recorded.spec_sha256 ?? '')) throw new Error('Existing spec provenance is incomplete or unsupported');
    if (recorded.source_repository !== sourceRepository || recorded.source_path !== input.sourcePath) throw new Error('Destination spec belongs to another source connection');
    if (!oldSpec || recorded.spec_sha256 !== sha256(oldSpec)) throw new Error('Destination spec has manual edits; reconcile them before source sync');
    if (oldSpec.equals(bytes)) { provenance = oldProvenance; effectiveSourceSha = recorded.source_sha; }
  } else if (branchRef && oldSpec) {
    throw new Error('Existing review branch spec has no provenance; reconcile or choose a new review branch');
  }
  const changed = !oldSpec?.equals(bytes) || !oldProvenance?.equals(provenance);
  if (changed) {
    const entries = [];
    for (const [file, content] of [[target, bytes], [PROVENANCE, provenance]]) {
      const blob = await api('POST', '/git/blobs', { content: content.toString('base64'), encoding: 'base64' });
      entries.push({ path: file, mode: '100644', type: 'blob', sha: blob.sha });
    }
    const tree = await api('POST', '/git/trees', { base_tree: commit.tree.sha, tree: entries });
    const updated = await api('POST', '/git/commits', { message: `Sync OpenAPI from ${sourceRepository}@${sourceSha.slice(0, 12)}`, tree: tree.sha, parents: [parent] });
    if (branchRef) await api('PATCH', `/git/refs/heads/${refPath(branch)}`, { sha: updated.sha, force: false });
    else { await api('POST', '/git/refs', { ref: `refs/heads/${branch}`, sha: updated.sha }); branchRef = { object: { sha: updated.sha } }; }
  }
  const prs = await api('GET', `/pulls?state=open&head=${encodeURIComponent(`${repository.split('/')[0]}:${branch}`)}&base=${encodeURIComponent(base)}`);
  if (!Array.isArray(prs)) throw new Error('Invalid GitHub pull-request response');
  const title = 'Sync OpenAPI specification';
  const body = `Update \`${target}\` from \`${sourceRepository}\` commit \`${effectiveSourceSha}\`.\n\nSpec SHA-256: \`${sha256(bytes)}\`. Provenance is recorded in \`${PROVENANCE}\`.\n\nReview this pull request before merging to regenerate the SDKs.`;
  let pr = prs[0];
  if (pr) pr = await api('PATCH', `/pulls/${pr.number}`, { title, body });
  else if (branchRef) {
    const comparison = await api('GET', `/compare/${encodeURIComponent(base)}...${encodeURIComponent(branch)}`);
    if (!Number.isInteger(comparison.ahead_by)) throw new Error('Invalid GitHub branch comparison response');
    if (comparison.ahead_by > 0) pr = await api('POST', '/pulls', { title, body, head: branch, base });
  }
  return { changed, pullRequestUrl: pr?.html_url ?? '', branch, specSha256: sha256(bytes) };
}
async function main() {
  const result = await syncSpec({ repository: process.env.POOLSTER_SPEC_REPOSITORY, sourcePath: process.env.POOLSTER_SPEC_SOURCE, targetPath: process.env.POOLSTER_SPEC_TARGET, token: process.env.POOLSTER_SPEC_TOKEN, branch: process.env.POOLSTER_SPEC_BRANCH, base: process.env.POOLSTER_SPEC_BASE, workspace: process.env.GITHUB_WORKSPACE, sourceRepository: process.env.GITHUB_REPOSITORY, sourceSha: process.env.GITHUB_SHA });
  if (process.env.GITHUB_OUTPUT) await appendFile(process.env.GITHUB_OUTPUT, `changed=${result.changed}\npull-request-url=${result.pullRequestUrl ? text(result.pullRequestUrl, 'pull request URL') : ''}\n`);
  console.log(JSON.stringify({ changed: result.changed, branch: result.branch, pullRequestUrl: result.pullRequestUrl }));
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main().catch(error => { console.error(error.message); process.exitCode = 1; });
