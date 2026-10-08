import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, mkdir, writeFile, readFile, symlink, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { preparePublish, publishPrepared, confirmPublication, runCommand } from '../publish.mjs';

const integrity = data => `sha512-${createHash('sha512').update(data).digest('base64')}`;
const checksum = data => createHash('sha256').update(data).digest('hex');
const response = (status, value = {}) => ({ status, json: async () => value });

async function fixture(t, registry, relative = 'sdk') {
  const workspace = await realpath(await mkdtemp(path.join(tmpdir(), 'poolster-publish-test-')));
  t.after(() => rm(workspace, { recursive: true, force: true }));
  const directory = path.join(workspace, relative);
  await mkdir(path.join(directory, '.poolster'), { recursive: true });
  const metadata = { schema_version: 1, name: 'release-component', version: '1.2.3', publisher: { registry, release_type: 'simple', commands: [] } };
  const saveMetadata = async () => writeFile(path.join(directory, '.poolster/package.json'), JSON.stringify(metadata));
  await saveMetadata();
  const calls = [];
  const git = async (program, args, options) => {
    calls.push({ program, args, options });
    if (program !== 'git') return undefined;
    return args.includes('--show-toplevel') ? workspace : 'release-commit';
  };
  return { workspace, directory, metadata, saveMetadata, calls, git,
    inputs: { registry, path: relative, workspace, tag: 'release-component-v1.2.3', temporaryRoot: workspace } };
}
async function npmFixture(t, { published = false, filename = 'package.tgz', entryPresent = true } = {}) {
  const fixture_ = await fixture(t, 'npm', 'sdk $() `literal`');
  const bytes = Buffer.from('exact built npm package');
  const manifest = { name: '@acme/example-sdk', version: '1.2.3', main: './dist/index.js', types: './dist/index.d.ts', exports: { '.': { types: './dist/index.d.ts', default: './dist/index.js' }, './*': './dist/*.js' } };
  await writeFile(path.join(fixture_.directory, 'package.json'), JSON.stringify(manifest));
  const state = { published, publishCalls: 0 };
  const run = async (program, args, options) => {
    const result = await fixture_.git(program, args, options);
    if (program === 'git') return result;
    assert.equal(program, 'npm');
    if (args[0] === 'pack') {
      assert.ok(args.includes('--ignore-scripts'));
      const staging = args[args.indexOf('--pack-destination') + 1];
      if (filename === path.basename(filename)) await writeFile(path.join(staging, filename), bytes);
      return JSON.stringify([{ name: manifest.name, version: manifest.version, filename, integrity: integrity(bytes), files: entryPresent ? [{ path: 'dist/index.js' }, { path: 'dist/index.d.ts' }] : [{ path: 'README.md' }] }]);
    }
    assert.equal(args[0], 'publish');
    assert.equal(options.cwd, fixture_.directory);
    assert.deepEqual(args.slice(2), ['--registry', 'https://registry.npmjs.org', '--access', 'public', '--provenance', '--ignore-scripts', '--tag', 'latest']);
    assert.deepEqual(await readFile(args[1]), bytes);
    state.publishCalls++;
    state.published = true;
    return '';
  };
  const fetch = async url => {
    assert.equal(url, 'https://registry.npmjs.org/%40acme%2Fexample-sdk/1.2.3');
    return state.published ? response(200, { name: manifest.name, version: manifest.version, dist: { integrity: integrity(bytes) } }) : response(404);
  };
  return { ...fixture_, bytes, manifest, state, dependencies: { run, fetch, sleep: async () => {} } };
}

test('npm packs and publishes exact bytes; shell characters in package path stay literal', async t => {
  const fixture_ = await npmFixture(t);
  const plan = await preparePublish(fixture_.inputs, fixture_.dependencies);
  assert.equal(plan.alreadyPublished, false);
  await publishPrepared(plan, fixture_.dependencies);
  assert.equal(fixture_.state.publishCalls, 1);
  assert.ok(fixture_.calls.every(call => call.options.cwd === fixture_.directory));
});
test('existing matching npm version skips publication; changed artifacts never pass', async t => {
  const fixture_ = await npmFixture(t, { published: true });
  const plan = await preparePublish(fixture_.inputs, fixture_.dependencies);
  assert.equal(plan.alreadyPublished, true);
  await publishPrepared(plan, fixture_.dependencies);
  assert.equal(fixture_.state.publishCalls, 0);
  await writeFile(plan.artifact, 'changed');
  await assert.rejects(publishPrepared(plan, fixture_.dependencies), /artifact changed/);
});
test('duplicate race succeeds only when registry integrity matches; auth failures remain errors', async t => {
  const fixture_ = await npmFixture(t);
  const plan = await preparePublish(fixture_.inputs, fixture_.dependencies);
  const run = async (program, args, options) => {
    if (program === 'npm' && args[0] === 'publish') { fixture_.state.published = true; throw new Error('duplicate upload'); }
    return fixture_.dependencies.run(program, args, options);
  };
  const result = await publishPrepared(plan, { ...fixture_.dependencies, run });
  assert.equal(result.alreadyPublished, true);
  fixture_.state.published = false;
  await assert.rejects(publishPrepared(plan, { ...fixture_.dependencies, run: async () => { throw new Error('authentication failed'); } }), /authentication failed/);
});
test('npm refuses missing compiled entrypoints, path traversal archives and mismatched content', async t => {
  const missing = await npmFixture(t, { entryPresent: false });
  await assert.rejects(preparePublish(missing.inputs, missing.dependencies), /entry point is missing/);
  const escaped = await npmFixture(t, { filename: '../escape.tgz' });
  await assert.rejects(preparePublish(escaped.inputs, escaped.dependencies), /one package archive/);
  const mismatch = await npmFixture(t);
  await assert.rejects(preparePublish(mismatch.inputs, { ...mismatch.dependencies, fetch: async () => response(200, { name: mismatch.manifest.name, version: '1.2.3', dist: { integrity: 'other' } }) }), /differs/);
});
test('invalid registry, wrong checkout/version, custom commands and lookup outages fail closed', async t => {
  const fixture_ = await npmFixture(t);
  await assert.rejects(preparePublish({ ...fixture_.inputs, registry: 'npm; echo secret' }, fixture_.dependencies), /Unsupported/);
  await assert.rejects(preparePublish({ ...fixture_.inputs, tag: 'release-v9.0.0' }, fixture_.dependencies), /tag does not match/);
  await assert.rejects(preparePublish({ ...fixture_.inputs, tag: 'v1.2.3\ninjected=value' }, fixture_.dependencies), /control characters/);
  await assert.rejects(preparePublish(fixture_.inputs, { ...fixture_.dependencies, run: async (program, args, options) => args.includes('HEAD') ? 'wrong-commit' : fixture_.dependencies.run(program, args, options) }), /exact released tag/);
  await assert.rejects(preparePublish(fixture_.inputs, { ...fixture_.dependencies, fetch: async () => response(403) }), /HTTP 403/);
  await assert.rejects(preparePublish(fixture_.inputs, { ...fixture_.dependencies, fetch: async () => response(503) }), /HTTP 503/);
  fixture_.metadata.publisher.commands = [{ program: 'custom-publish', args: [] }];
  await fixture_.saveMetadata();
  await assert.rejects(preparePublish(fixture_.inputs, fixture_.dependencies), /custom publishing commands/);
});
test('package and distribution symlinks cannot escape their workspace', async t => {
  const fixture_ = await npmFixture(t);
  const outside = await mkdtemp(path.join(tmpdir(), 'poolster-publish-outside-'));
  t.after(() => rm(outside, { recursive: true, force: true }));
  await symlink(outside, path.join(fixture_.workspace, 'outside'));
  await assert.rejects(preparePublish({ ...fixture_.inputs, path: 'outside' }, fixture_.dependencies), /escapes/);
});

test('Cargo publishes after packaging, validates exact archive digest, and rejects publish=false', async t => {
  const fixture_ = await fixture(t, 'crates.io');
  await writeFile(path.join(fixture_.directory, 'Cargo.toml'), '[package]\nname="example-sdk"\nversion="1.2.3"\n');
  const target = path.join(fixture_.workspace, 'target');
  await mkdir(path.join(target, 'package'), { recursive: true });
  const bytes = Buffer.from('exact crate bytes');
  let published = false;
  let allowed = null;
  const run = async (program, args, options) => {
    const git = await fixture_.git(program, args, options);
    if (program === 'git') return git;
    assert.equal(program, 'cargo');
    if (args[0] === 'metadata') return JSON.stringify({ target_directory: target, packages: [{ name: 'example-sdk', version: '1.2.3', manifest_path: path.join(fixture_.directory, 'Cargo.toml'), publish: allowed }] });
    if (args[0] === 'package') { await writeFile(path.join(target, 'package/example-sdk-1.2.3.crate'), bytes); return ''; }
    assert.equal(args[0], 'publish');
    assert.equal(args[args.indexOf('--registry') + 1], 'crates-io');
    published = true;
    return '';
  };
  const fetch = async url => { assert.equal(url, 'https://crates.io/api/v1/crates/example-sdk/1.2.3'); return published ? response(200, { version: { num: '1.2.3', checksum: checksum(bytes) } }) : response(404); };
  const dependencies = { run, fetch, sleep: async () => {} };
  const plan = await preparePublish(fixture_.inputs, dependencies);
  await publishPrepared(plan, dependencies);
  assert.equal(published, true);
  allowed = [];
  await assert.rejects(preparePublish(fixture_.inputs, dependencies), /does not allow/);
});

test('PyPI partial publication resumes only missing matching files and verifies every final digest', async t => {
  const fixture_ = await fixture(t, 'pypi');
  const dist = path.join(fixture_.directory, 'dist');
  await mkdir(dist);
  await writeFile(path.join(fixture_.directory, 'pyproject.toml'), '[project]\nname="Example.SDK"\nversion="1.2.3"\n');
  const files = [{ filename: 'example_sdk-1.2.3-py3-none-any.whl', data: Buffer.from('wheel') }, { filename: 'example_sdk-1.2.3.tar.gz', data: Buffer.from('sdist') }];
  for (const file of files) await writeFile(path.join(dist, file.filename), file.data);
  let published = [files[0]];
  const run = async (program, args, options) => {
    const git = await fixture_.git(program, args, options);
    if (program === 'git') return git;
    assert.equal(program, 'python3');
    assert.equal(args[0], '-c');
    assert.match(args[1], /tomllib/);
    return JSON.stringify({ name: 'Example.SDK', version: '1.2.3', files: files.map(file => ({ filename: file.filename, name: 'example-sdk', version: '1.2.3' })) });
  };
  const fetch = async url => { assert.equal(url, 'https://pypi.org/pypi/Example.SDK/1.2.3/json'); return response(200, { urls: published.map(file => ({ filename: file.filename, digests: { sha256: checksum(file.data) } })) }); };
  const dependencies = { run, fetch, sleep: async () => {} };
  const plan = await preparePublish(fixture_.inputs, dependencies);
  assert.equal(plan.alreadyPublished, false);
  await assert.rejects(publishPrepared(plan, dependencies), /official PyPA action/);
  published = files;
  await confirmPublication(plan, dependencies);
  const duplicate = await preparePublish(fixture_.inputs, dependencies);
  assert.equal(duplicate.alreadyPublished, true);
  published = [{ ...files[0], data: Buffer.from('different wheel') }];
  await assert.rejects(preparePublish(fixture_.inputs, dependencies), /differs/);
});
test('PyPI refuses mixed distribution identity', async t => {
  const fixture_ = await fixture(t, 'pypi');
  const dist = path.join(fixture_.directory, 'dist');
  await mkdir(dist);
  await writeFile(path.join(dist, 'wrong.whl'), 'bytes');
  const run = async (program, args, options) => program === 'git' ? fixture_.git(program, args, options) : JSON.stringify({ name: 'correct-sdk', version: '1.2.3', files: [{ filename: 'wrong.whl', name: 'wrong-sdk', version: '1.2.3' }] });
  await assert.rejects(preparePublish(fixture_.inputs, { run }), /distribution metadata/);
});

for (const [relative, module, version, expected] of [
  ['.', 'github.com/acme/sdk', '1.2.3', 'v1.2.3'],
  ['generated/go', 'github.com/acme/sdk/generated/go', '1.2.3', 'generated/go/v1.2.3'],
  ['generated/go/v2', 'github.com/acme/sdk/generated/go/v2', '2.3.4', 'generated/go/v2.3.4'],
  ['v2', 'github.com/acme/sdk/v2', '2.3.4', 'v2.3.4'],
]) test(`Go uses canonical module-source tag ${expected}`, async t => {
  const fixture_ = await fixture(t, 'go', relative);
  fixture_.metadata.version = version;
  await fixture_.saveMetadata();
  const state = { published: false };
  const run = async (program, args, options) => {
    const git = await fixture_.git(program, args, options);
    if (program === 'git') return git;
    assert.equal(program, 'go');
    if (args[0] === 'mod') return JSON.stringify({ Module: { Path: module } });
    assert.deepEqual(args, ['list', '-m', '-json', `${module}@v${version}`]);
    assert.equal(options.env.GOPROXY, 'https://proxy.golang.org');
    state.published = true;
    return JSON.stringify({ Path: module, Version: `v${version}` });
  };
  const dependencies = { run, fetch: async () => state.published ? response(200, { Version: `v${version}` }) : response(404), sleep: async () => {} };
  const plan = await preparePublish({ ...fixture_.inputs, tag: expected }, dependencies);
  await publishPrepared(plan, dependencies);
  assert.equal(state.published, true);
  if (relative !== '.') await assert.rejects(preparePublish({ ...fixture_.inputs, tag: `component-v${version}` }, dependencies), /requires the source tag/);
});
test('Go rejects a major-version/import-path mismatch', async t => {
  const fixture_ = await fixture(t, 'go', 'go');
  fixture_.metadata.version = '2.0.0'; await fixture_.saveMetadata();
  const run = async (program, args, options) => program === 'git' ? fixture_.git(program, args, options) : JSON.stringify({ Module: { Path: 'github.com/acme/sdk/go' } });
  await assert.rejects(preparePublish({ ...fixture_.inputs, tag: 'go/v2.0.0' }, { run }), /semantic import path/);
});

test('subprocess failures never expose captured credentials or output', async () => {
  await assert.rejects(runCommand(process.execPath, ['-e', 'console.error("super-secret-token"); process.exit(2)']), error => {
    assert.doesNotMatch(error.message, /super-secret-token/);
    assert.match(error.message, /failed \(exit 2\)/);
    return true;
  });
});
test('vendored composite uses official identity actions and passes inputs through environment', async () => {
  const action = await readFile(new URL('../action.yml', import.meta.url), 'utf8');
  assert.match(action, /node-version: '24'/);
  assert.match(action, /rust-lang\/crates-io-auth-action@v1/);
  assert.match(action, /pypa\/gh-action-pypi-publish@release\/v1/);
  assert.match(action, /skip-existing: 'true'/);
  assert.match(action, /POOLSTER_PUBLISH_TAG: \$\{\{ inputs\.tag \}\}/);
  for (const line of action.split('\n').filter(line => /^\s+run:/.test(line))) {
    assert.doesNotMatch(line, /\$\{\{/);
    assert.match(line, /node "\$POOLSTER_ACTION_PATH\/publish\.mjs" (?:prepare|publish|confirm)/);
  }
});


test('actual Python reader validates static TOML and wheel/sdist metadata without extraction', async t => {
  const python = process.env.POOLSTER_TEST_PYTHON ?? 'python3';
  try { await runCommand(python, ['-c', 'import tomllib']); }
  catch { t.skip('requires Python 3.11+ (set POOLSTER_TEST_PYTHON to a compatible interpreter)'); return; }
  const fixture_ = await fixture(t, 'pypi');
  await writeFile(path.join(fixture_.directory, 'pyproject.toml'), '[project]\nname="example-sdk"\nversion="1.2.3"\n');
  const dist = path.join(fixture_.directory, 'dist');
  await mkdir(dist);
  const create = `import io, pathlib, sys, tarfile, zipfile
root = pathlib.Path(sys.argv[1])
metadata = b'Metadata-Version: 2.1\\nName: example-sdk\\nVersion: 1.2.3\\n'
with zipfile.ZipFile(root / 'example_sdk-1.2.3-py3-none-any.whl', 'w') as archive:
    archive.writestr('example_sdk-1.2.3.dist-info/METADATA', metadata)
with tarfile.open(root / 'example_sdk-1.2.3.tar.gz', 'w:gz') as archive:
    record = tarfile.TarInfo('example_sdk-1.2.3/PKG-INFO')
    record.size = len(metadata)
    archive.addfile(record, io.BytesIO(metadata))
`;
  await runCommand(python, ['-c', create, dist]);
  const run = async (program, args, options) => program === 'git' ? fixture_.git(program, args, options) : runCommand(python, args, options);
  const plan = await preparePublish(fixture_.inputs, { run, fetch: async () => response(404) });
  assert.equal(plan.files.length, 2);
  assert.equal(plan.name, 'example-sdk');
  await writeFile(path.join(dist, 'unrelated.txt'), 'unrelated');
  await assert.rejects(preparePublish(fixture_.inputs, { run, fetch: async () => response(404) }), /failed/);
});
