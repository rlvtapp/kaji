const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const toml = require('@iarna/toml');
const { GenericToml } = require('release-please/build/src/updaters/generic-toml');
const { GenericJson } = require('release-please/build/src/updaters/generic-json');
const { Generic } = require('release-please/build/src/updaters/generic');
const { Version } = require('release-please/build/src/version');
const { DefaultVersioningStrategy } = require('release-please/build/src/versioning-strategies/default');

const root = path.join(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const config = JSON.parse(read('release-please-config.json')).packages['.'];
const version = Version.parse('0.5.1');

test('release updates inherited workspace and lock versions without changing dependencies', () => {
  assert.equal(config['release-type'], 'simple');
  assert.equal(read(config['version-file']).trim(), toml.parse(read('Cargo.toml')).workspace.package.version);
  const updated = new Map();
  for (const extra of config['extra-files']) {
    const updater = extra.type === 'toml' ? new GenericToml(extra.jsonpath, version)
      : extra.type === 'json' ? new GenericJson(extra.jsonpath, version)
      : new Generic({ version });
    updated.set(extra.path, updater.updateContent(updated.get(extra.path) ?? read(extra.path)));
  }
  const manifest = toml.parse(updated.get('Cargo.toml'));
  assert.equal(manifest.workspace.package.version, '0.5.1');
  const names = new Set(manifest.workspace.members.map(member => {
    const memberManifest = toml.parse(read(`${member}/Cargo.toml`));
    assert.deepEqual(memberManifest.package.version, { workspace: true });
    return memberManifest.package.name;
  }));
  const before = toml.parse(read('Cargo.lock'));
  const after = toml.parse(updated.get('Cargo.lock'));
  let changed = 0;
  for (let i = 0; i < before.package.length; i++) {
    const expected = { ...before.package[i] };
    if (names.has(expected.name)) {
      expected.version = '0.5.1';
      changed++;
    }
    assert.deepEqual(after.package[i], expected);
  }
  assert.equal(changed, names.size);
  for (const member of manifest.workspace.members) {
    const beforeManifest = toml.parse(read(`${member}/Cargo.toml`));
    const dependencies = Object.entries(beforeManifest.dependencies ?? {}).filter(([, dep]) => dep.path);
    if (!dependencies.length) continue;
    const afterManifest = toml.parse(updated.get(`${member}/Cargo.toml`));
    for (const [name, dep] of dependencies) {
      assert.equal(dep.version, read('version.txt').trim());
      assert.deepEqual(afterManifest.dependencies[name], { ...dep, version: '0.5.1' });
    }
  }
  const launcher = JSON.parse(updated.get('packages/npm/cli/package.json'));
  assert.equal(launcher.version, '0.5.1');
  for (const value of Object.values(launcher.optionalDependencies)) assert.equal(value, '0.5.1');
  const sdk = JSON.parse(updated.get('packages/npm/sdk/package.json'));
  assert.equal(sdk.version, '0.5.1');
  for (const value of Object.values(sdk.optionalDependencies)) assert.equal(value, '0.5.1');
  assert.match(updated.get('packages/python/setup.py'), /version="0\.5\.1"/);
});

test('Python prerelease markers update completely when returning to stable releases', () => {
  const source = read('packages/python/setup.py');
  const prerelease = new Generic({ version: Version.parse('0.5.0-alpha.2') }).updateContent(source);
  assert.match(prerelease, /version="0\.5\.0-alpha\.2"/);
  const stable = new Generic({ version }).updateContent(prerelease);
  assert.match(stable, /version="0\.5\.1"/);
  assert.doesNotMatch(stable, /version="0\.5\.1(?:a|b|rc|-)\d/);
});

test('CLI native tests build the compiler first and publication requires a release tag', () => {
  const ci = read('.github/workflows/ci.yml');
  const build = ci.indexOf('go build -o ../target/debug/poolster-openapi .');
  const probe = ci.indexOf('cargo test -p poolster-cli --test local_references');
  assert.ok(build >= 0 && probe > build);
  const release = read('.github/workflows/release-please.yml');
  assert.doesNotMatch(release, /skip-github-release|POOLSTER_RELEASE_ENABLED/);
  assert.match(release, /on:\n  push:\n    branches: \[main\]/);
  const publish = read('.github/workflows/npm-publish.yml');
  assert.match(publish, /publish:\n    name: Publish to npm\n    needs: \[verify, generated-sdk, native, launchers\]\n    if: startsWith\(github\.ref, 'refs\/tags\/v'\)/);
  assert.match(publish, /publish-python:\n    name: Publish to PyPI\n    needs: \[verify, generated-sdk, native, launchers\]\n    if: startsWith\(github\.ref, 'refs\/tags\/v'\)/);
  assert.match(publish, /publish-crates:\n    name: Publish Rust SDK to crates.io\n    needs: \[verify, generated-sdk, native, launchers\]\n    if: startsWith\(github\.ref, 'refs\/tags\/v'\)/);
  assert.match(publish, /CARGO_REGISTRY_TOKEN: \$\{\{ secrets\.CRATES_IO_TOKEN \}\}/);
  assert.match(publish, /python scripts\/publish-crates.py --tag "\$GITHUB_REF_NAME"/);
});

test('feature releases advance minor versions before 1.0', () => {
  const strategy = new DefaultVersioningStrategy({
    bumpMinorPreMajor: config['bump-minor-pre-major'],
    bumpPatchForMinorPreMajor: config['bump-patch-for-minor-pre-major'],
  });
  const previous = Version.parse('0.4.0');
  const next = strategy.bump(previous, [{ type: 'feat', breaking: false, notes: [] }]);
  assert.equal(next.toString(), '0.5.0');
});
