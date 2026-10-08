const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const template = fs.readFileSync(path.join(__dirname, '..', 'poolster.yml'), 'utf8');

test('GitLab template uses a glibc Node image and invokes the npm launcher', () => {
  assert.match(template, /image: node:22-bookworm-slim/);
  assert.doesNotMatch(template, /node:22-alpine/);
  assert.match(template, /npx --yes "poolster@\$POOLSTER_VERSION" generate --config "\$POOLSTER_CONFIG"/);
});
