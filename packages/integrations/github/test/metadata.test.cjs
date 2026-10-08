const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const metadata = fs.readFileSync(path.join(__dirname, '..', 'action.yml'), 'utf8');

test('composite action installs the published launcher without shell-interpolating inputs', () => {
  assert.match(metadata, /^runs:\n  using: composite$/m);
  assert.match(metadata, /uses: actions\/setup-node@v4/);
  assert.match(metadata, /node-version: 22/);
  assert.match(metadata, /POOLSTER_CONFIG: \$\{\{ inputs\.config \}\}/);
  assert.match(metadata, /POOLSTER_VERSION: \$\{\{ inputs\.version \}\}/);
  assert.match(metadata, /npx --yes "poolster@\$POOLSTER_VERSION" generate --config "\$POOLSTER_CONFIG"/);
});
