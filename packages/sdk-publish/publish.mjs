#!/usr/bin/env node
// Standard SDK publishers. Inputs are data; commands use argument vectors.
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, writeFile, readdir, realpath, mkdir, appendFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const execFileAsync = promisify(execFile);
const REGISTRIES = new Set(['npm', 'pypi', 'crates.io', 'go']);
const PUBLIC_NPM = 'https://registry.npmjs.org';
const SEMVER = /^[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?(?:\+[A-Za-z0-9.-]+)?$/;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

export async function runCommand(program, args, options = {}) {
  try {
    const result = await execFileAsync(program, args, {
      cwd: options.cwd, env: { ...process.env, ...options.env },
      maxBuffer: 16 * 1024 * 1024, timeout: 15 * 60 * 1000,
    });
    return result.stdout.trim();
  } catch (error) {
    // Package managers can include credentials in diagnostics. Never echo their
    // captured output or a full environment; registry verification handles races.
    const code = Number.isInteger(error.code) ? error.code : 'unavailable';
    throw new Error(`${program} failed (exit ${code}); inspect the package configuration and registry authentication`);
  }
}

function cleanString(value, label) {
  if (typeof value !== 'string' || !value || /[\x00-\x1f\x7f]/.test(value)) {
    throw new Error(`${label} must be a nonempty string without control characters`);
  }
  return value;
}
function inside(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative));
}
async function contained(root, candidate) {
  const resolved = await realpath(candidate);
  if (!inside(root, resolved)) throw new Error('Package or artifact path escapes the workspace');
  return resolved;
}
const hash = (data, algorithm = 'sha256') => createHash(algorithm).update(data).digest('hex');
function npmIntegrity(data) { return `sha512-${createHash('sha512').update(data).digest('base64')}`; }
function escapedModule(value) { return value.replace(/[A-Z]/g, character => `!${character.toLowerCase()}`); }

// The generated Python manifest is static PEP 621. Dynamic metadata requires a
// custom publisher. Reading archive metadata does not extract or execute code.
const PYTHON_INFO = String.raw`
import email.parser, json, pathlib, sys, tarfile, tomllib, zipfile
root = pathlib.Path(sys.argv[1])
project = tomllib.loads((root / 'pyproject.toml').read_text())['project']
if 'name' not in project or 'version' not in project or 'version' in project.get('dynamic', []):
    raise ValueError('standard PyPI publishing requires static project name/version')
files = []
for item in sorted(pathlib.Path(sys.argv[2]).iterdir()):
    if item.name.endswith('.whl'):
        with zipfile.ZipFile(item) as archive:
            records = [entry for entry in archive.infolist() if entry.filename.endswith('.dist-info/METADATA')]
            if len(records) != 1 or records[0].file_size > 1048576:
                raise ValueError('wheel requires one bounded METADATA record')
            text = archive.read(records[0]).decode('utf-8')
    elif item.name.endswith('.tar.gz'):
        with tarfile.open(item, 'r:gz') as archive:
            records = [entry for entry in archive.getmembers() if entry.name.count('/') == 1 and entry.name.endswith('/PKG-INFO')]
            if len(records) != 1 or not records[0].isfile() or records[0].size > 1048576:
                raise ValueError('sdist requires one bounded PKG-INFO record')
            text = archive.extractfile(records[0]).read().decode('utf-8')
    else:
        raise ValueError('distribution directory must contain only wheels and .tar.gz sdists')
    metadata = email.parser.Parser().parsestr(text)
    if len(metadata.get_all('Name', [])) != 1 or len(metadata.get_all('Version', [])) != 1:
        raise ValueError('distribution metadata requires one name and version')
    files.append({'filename': item.name, 'name': metadata['Name'], 'version': metadata['Version']})
print(json.dumps({'name': project['name'], 'version': project['version'], 'files': files}))
`;
function pythonName(value) { return value.toLowerCase().replace(/[-_.]+/g, '-'); }

function npmTargets(manifest) {
  const targets = [];
  const collect = value => {
    if (typeof value === 'string') targets.push(value);
    else if (Array.isArray(value)) value.forEach(collect);
    else if (value && typeof value === 'object') Object.values(value).forEach(collect);
  };
  [manifest.main, manifest.types, manifest.typings, manifest.bin, manifest.exports].forEach(collect);
  return targets;
}
function verifyNpmEntryPoints(manifest, files) {
  if (!Array.isArray(files)) throw new Error('npm pack did not describe its package files');
  for (const target of npmTargets(manifest)) {
    const normalized = cleanString(target, 'npm entry point').replace(/^\.\//, '');
    if (normalized.startsWith('/') || normalized.split('/').includes('..')) throw new Error('npm entry point escapes the package');
    const pattern = new RegExp(`^${normalized.split('*').map(part => part.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('.*')}$`);
    if (!files.some(file => typeof file.path === 'string' && pattern.test(file.path))) {
      throw new Error('A declared npm entry point is missing from the packed files; build the SDK before publishing');
    }
  }
}

export async function registryJson(url, fetcher = fetch) {
  const response = await fetcher(url, {
    headers: { 'User-Agent': 'poolster-sdk-publish/1', 'Cache-Control': 'no-cache' },
    signal: AbortSignal.timeout(15000), redirect: 'error',
  });
  if (response.status === 404) return null;
  if (response.status !== 200) throw new Error(`Registry lookup failed with HTTP ${response.status}`);
  return response.json();
}

async function publishedState(plan, dependencies = {}) {
  const get = url => registryJson(url, dependencies.fetch ?? fetch);
  if (plan.registry === 'npm') {
    const data = await get(`${PUBLIC_NPM}/${encodeURIComponent(plan.name)}/${encodeURIComponent(plan.version)}`);
    if (!data) return false;
    if (data.name !== plan.name || data.version !== plan.version || data.dist?.integrity !== plan.integrity) {
      throw new Error('Published npm version differs from the locally built artifact');
    }
    return true;
  }
  if (plan.registry === 'crates.io') {
    const data = await get(`https://crates.io/api/v1/crates/${encodeURIComponent(plan.name)}/${encodeURIComponent(plan.version)}`);
    if (!data) return false;
    if (data.version?.num !== plan.version || data.version?.checksum !== plan.checksum) {
      throw new Error('Published crate version differs from the locally packaged artifact');
    }
    return true;
  }
  if (plan.registry === 'pypi') {
    const data = await get(`https://pypi.org/pypi/${encodeURIComponent(plan.name)}/${encodeURIComponent(plan.version)}/json`);
    if (!data) return false;
    const files = new Map((data.urls ?? []).map(file => [file.filename, file.digests?.sha256]));
    let complete = true;
    for (const file of plan.files) {
      if (!files.has(file.filename)) { complete = false; continue; }
      if (files.get(file.filename) !== file.checksum) {
        throw new Error('Published PyPI distribution differs from the locally built artifact');
      }
    }
    return complete;
  }
  const data = await get(`https://proxy.golang.org/${escapedModule(plan.name)}/@v/${escapedModule(`v${plan.version}`)}.info`);
  if (!data) return false;
  if (data.Version !== `v${plan.version}`) throw new Error('Go proxy returned an unexpected module version');
  return true;
}

export async function preparePublish(inputs, dependencies = {}) {
  const registry = cleanString(inputs.registry, 'registry');
  if (!REGISTRIES.has(registry)) throw new Error('Unsupported standard registry; use poolster sdk run --phase publish for custom publishers');
  const run = dependencies.run ?? runCommand;
  const workspace = await realpath(inputs.workspace ?? process.env.GITHUB_WORKSPACE ?? process.cwd());
  const directory = await contained(workspace, path.resolve(workspace, cleanString(inputs.path, 'path')));
  const tag = cleanString(inputs.tag, 'tag');
  if (tag.startsWith('-')) throw new Error('Release tag cannot start with a dash');
  const metadataPath = await contained(workspace, path.join(directory, '.poolster/package.json'));
  const metadata = JSON.parse(await readFile(metadataPath, 'utf8'));
  if (metadata.schema_version !== 1 || metadata.publisher?.registry !== registry) throw new Error('Package publisher metadata must explicitly match the selected registry');
  if (metadata.publisher.commands?.length) throw new Error('Package declares custom publishing commands; use poolster sdk run --phase publish');
  const version = cleanString(metadata.version, 'package metadata version').replace(/^v(?=\d)/, '');
  const pythonVersion = /^[0-9]+\.[0-9]+\.[0-9]+(?:(?:a|b|rc)[0-9]+|\.post[0-9]+|\.dev[0-9]+)?(?:\+[A-Za-z0-9.-]+)?$/;
  if (!(registry === 'pypi' ? pythonVersion.test(version) : SEMVER.test(version))) throw new Error('Standard publishing requires a semantic package version (normalized PEP 440 for PyPI)');
  if (!(tag === `v${version}` || tag.endsWith(`-v${version}`) || tag.endsWith(`/v${version}`))) {
    throw new Error('Release tag does not match the package metadata version');
  }
  const head = await run('git', ['rev-parse', 'HEAD'], { cwd: directory });
  const release = await run('git', ['rev-parse', `refs/tags/${tag}^{commit}`], { cwd: directory });
  if (head !== release) throw new Error('Checkout must be the exact released tag commit');
  const temporaryRoot = inputs.temporaryRoot ?? process.env.RUNNER_TEMP ?? workspace;
  const staging = path.join(temporaryRoot, `poolster-publish-${randomUUID()}`);
  await mkdir(staging, { recursive: true });
  const plan = { schema_version: 1, registry, directory, workspace, tag, version, staging };
  if (registry === 'npm') {
    const manifest = JSON.parse(await readFile(await contained(workspace, path.join(directory, 'package.json')), 'utf8'));
    plan.name = cleanString(manifest.name, 'npm package name');
    if (!/^(?:@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*$/.test(plan.name) || manifest.private === true) throw new Error('Standard npm publishing requires a public publishable package name');
    if (manifest.version !== version) throw new Error('npm manifest version does not match package metadata');
    if (manifest.publishConfig?.registry && manifest.publishConfig.registry.replace(/\/$/, '') !== PUBLIC_NPM) throw new Error('Standard npm publisher only targets registry.npmjs.org');
    plan.npmTag = inputs.npmTag ?? 'latest';
    if (!/^[A-Za-z][A-Za-z0-9._-]*$/.test(plan.npmTag)) throw new Error('Invalid npm distribution tag');
    const packed = JSON.parse(await run('npm', ['pack', '--json', '--ignore-scripts', '--pack-destination', staging], { cwd: directory }));
    if (!Array.isArray(packed) || packed.length !== 1 || path.basename(packed[0].filename) !== packed[0].filename) throw new Error('npm pack must produce one package archive');
    verifyNpmEntryPoints(manifest, packed[0].files);
    plan.artifact = await contained(await realpath(staging), path.join(staging, packed[0].filename));
    plan.integrity = npmIntegrity(await readFile(plan.artifact));
    if (packed[0].name !== plan.name || packed[0].version !== version || packed[0].integrity !== plan.integrity) throw new Error('npm pack metadata or integrity does not match the package');
  } else if (registry === 'crates.io') {
    const manifestPath = await contained(workspace, path.join(directory, 'Cargo.toml'));
    const cargo = JSON.parse(await run('cargo', ['metadata', '--no-deps', '--format-version', '1', '--manifest-path', manifestPath], { cwd: directory }));
    const candidates = cargo.packages.filter(item => path.resolve(item.manifest_path) === manifestPath);
    if (candidates.length !== 1) throw new Error('Select a single Cargo package directory, not a workspace root');
    const crate = candidates[0];
    plan.name = cleanString(crate.name, 'crate name');
    if (!/^[A-Za-z0-9][A-Za-z0-9_-]*$/.test(plan.name) || crate.version !== version) throw new Error('Cargo name/version does not match a publishable package');
    if (Array.isArray(crate.publish) && !crate.publish.includes('crates-io')) throw new Error('Cargo package does not allow publishing to crates.io');
    await run('cargo', ['package', '--manifest-path', manifestPath, '--allow-dirty'], { cwd: directory });
    // Cargo target directories can live outside the checkout; copy only the
    // expected artifact to our staging directory and recheck its digest later.
    const source = path.join(cargo.target_directory, 'package', `${plan.name}-${version}.crate`);
    const data = await readFile(source);
    plan.artifact = path.join(staging, `${plan.name}-${version}.crate`);
    await writeFile(plan.artifact, data);
    plan.checksum = hash(data);
    plan.manifestPath = manifestPath;
  } else if (registry === 'pypi') {
    plan.distributionDirectory = await contained(workspace, path.resolve(directory, inputs.distDir ?? 'dist'));
    if (!inside(directory, plan.distributionDirectory)) throw new Error('PyPI distributions must be inside the selected package');
    for (const filename of await readdir(plan.distributionDirectory)) {
      await contained(plan.distributionDirectory, path.join(plan.distributionDirectory, filename));
    }
    const python = JSON.parse(await run('python3', ['-c', PYTHON_INFO, directory, plan.distributionDirectory], { cwd: directory }));
    plan.name = cleanString(python.name, 'PyPI project name');
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(plan.name) || python.version !== version) throw new Error('PyPI manifest version/name does not match package metadata');
    if (!python.files.length) throw new Error('Build wheels/sdists before publishing; distribution directory is empty');
    plan.files = [];
    for (const file of python.files) {
      if (pythonName(file.name) !== pythonName(plan.name) || file.version !== version || path.basename(file.filename) !== file.filename) throw new Error('PyPI distribution metadata does not match this package release');
      const filename = await contained(plan.distributionDirectory, path.join(plan.distributionDirectory, file.filename));
      plan.files.push({ filename: file.filename, checksum: hash(await readFile(filename)) });
    }
  } else {
    const go = JSON.parse(await run('go', ['mod', 'edit', '-json'], { cwd: directory }));
    plan.name = cleanString(go.Module?.Path, 'Go module path');
    if (!/^[A-Za-z0-9][A-Za-z0-9._~!/-]*$/.test(plan.name) || plan.name.split('/').some(part => !part || part === '.' || part === '..')) throw new Error('Invalid Go module path');
    const major = Number(version.split('.')[0]);
    const suffix = plan.name.match(/\/v([2-9][0-9]*)$/);
    if ((major >= 2 && Number(suffix?.[1]) !== major) || (major < 2 && suffix)) throw new Error('Go module major version must match its semantic import path');
    const gitRoot = await realpath(await run('git', ['rev-parse', '--show-toplevel'], { cwd: directory }));
    let prefix = path.relative(gitRoot, directory).split(path.sep).join('/');
    if (suffix && prefix.endsWith(`/v${major}`)) prefix = prefix.slice(0, -(String(major).length + 2));
    if (suffix && prefix === `v${major}`) prefix = '';
    const expectedTag = `${prefix ? `${prefix}/` : ''}v${version}`;
    if (tag !== expectedTag) throw new Error(`Go module requires the source tag ${expectedTag}`);
  }
  plan.alreadyPublished = await publishedState(plan, dependencies);
  return plan;
}

async function verifyLocalArtifacts(plan) {
  if (plan.registry === 'npm' && npmIntegrity(await readFile(plan.artifact)) !== plan.integrity) throw new Error('npm artifact changed after preparation');
  if (plan.registry === 'crates.io' && hash(await readFile(plan.artifact)) !== plan.checksum) throw new Error('Cargo artifact changed after preparation');
  if (plan.registry === 'pypi') for (const file of plan.files) {
    if (hash(await readFile(path.join(plan.distributionDirectory, file.filename))) !== file.checksum) throw new Error('PyPI artifact changed after preparation');
  }
}
async function confirmPublished(plan, dependencies) {
  for (let attempt = 0; attempt < 3; attempt++) {
    if (await publishedState(plan, dependencies)) return true;
    if (attempt < 2) await (dependencies.sleep ?? sleep)(1000 * (attempt + 1));
  }
  return false;
}

export async function publishPrepared(plan, dependencies = {}) {
  if (plan.schema_version !== 1 || !REGISTRIES.has(plan.registry)) throw new Error('Invalid publication state');
  await verifyLocalArtifacts(plan);
  if (plan.alreadyPublished) return { alreadyPublished: true, name: plan.name, version: plan.version };
  if (plan.registry === 'pypi') throw new Error('Use the official PyPA action to upload prepared PyPI distributions');
  const run = dependencies.run ?? runCommand;
  let failure;
  try {
    if (plan.registry === 'npm') {
      await run('npm', ['publish', plan.artifact, '--registry', PUBLIC_NPM, '--access', 'public', '--provenance', '--ignore-scripts', '--tag', plan.npmTag], { cwd: plan.directory });
    } else if (plan.registry === 'crates.io') {
      await run('cargo', ['publish', '--registry', 'crates-io', '--manifest-path', plan.manifestPath, '--allow-dirty'], { cwd: plan.directory });
    } else {
      const result = JSON.parse(await run('go', ['list', '-m', '-json', `${plan.name}@v${plan.version}`], { cwd: plan.directory, env: { GOPROXY: 'https://proxy.golang.org', GOSUMDB: 'sum.golang.org', GOWORK: 'off' } }));
      if (result.Path !== plan.name || result.Version !== `v${plan.version}`) throw new Error('Go proxy resolved a different module release');
    }
  } catch (error) { failure = error; }
  // Only exact artifact verification can turn an uncertain/duplicate upload
  // into success. Authentication, network and unrelated errors stay failures.
  if (!await confirmPublished(plan, dependencies)) {
    if (failure) throw failure;
    throw new Error('Publication could not be confirmed by the registry');
  }
  return { alreadyPublished: Boolean(failure), name: plan.name, version: plan.version };
}

export async function confirmPublication(plan, dependencies = {}) {
  if (plan.schema_version !== 1 || !REGISTRIES.has(plan.registry)) throw new Error('Invalid publication state');
  await verifyLocalArtifacts(plan);
  if (!await confirmPublished(plan, dependencies)) throw new Error('Publication could not be confirmed by the registry');
}

async function writeOutputs(plan, statePath) {
  const fields = { state: statePath, registry: plan.registry, name: plan.name, version: plan.version, 'already-published': String(plan.alreadyPublished) };
  if (plan.distributionDirectory) fields['packages-dir'] = path.relative(plan.workspace, plan.distributionDirectory).split(path.sep).join('/');
  const data = Object.entries(fields).map(([key, value]) => `${key}=${cleanString(value, key)}\n`).join('');
  if (process.env.GITHUB_OUTPUT) await appendFile(process.env.GITHUB_OUTPUT, data);
  console.log(JSON.stringify({ registry: plan.registry, name: plan.name, version: plan.version, alreadyPublished: plan.alreadyPublished }));
}

export async function main(arguments_ = process.argv.slice(2)) {
  const phase = arguments_[0] ?? 'prepare';
  if (phase === 'prepare' || phase === 'execute') {
    const plan = await preparePublish({ registry: process.env.POOLSTER_PUBLISH_REGISTRY, path: process.env.POOLSTER_PUBLISH_PATH, tag: process.env.POOLSTER_PUBLISH_TAG, npmTag: process.env.POOLSTER_NPM_TAG || 'latest', distDir: process.env.POOLSTER_DIST_DIR || 'dist' });
    const statePath = path.join(plan.staging, 'state.json');
    await writeFile(statePath, JSON.stringify(plan, null, 2));
    await writeOutputs(plan, statePath);
    if (phase === 'execute') await publishPrepared(plan);
    return;
  }
  const state = JSON.parse(await readFile(cleanString(process.env.POOLSTER_PUBLISH_STATE, 'publication state path'), 'utf8'));
  if (phase === 'publish') { await publishPrepared(state); return; }
  if (phase === 'confirm') {
    await confirmPublication(state);
    return;
  }
  throw new Error('Phase must be prepare, publish, confirm, or execute');
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => { console.error(`SDK publication failed: ${error.message}`); process.exitCode = 1; });
}
