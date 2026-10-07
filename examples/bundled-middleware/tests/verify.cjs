const assert = require('node:assert/strict')
const { execFileSync } = require('node:child_process')
const { mkdtempSync, rmSync } = require('node:fs')
const { tmpdir } = require('node:os')
const { join } = require('node:path')

// Compile to an isolated CommonJS test directory; the published package keeps
// its normal ESM output for bundler consumers. Do not modify generated sources.
const output = mkdtempSync(join(tmpdir(), 'notes-sdk-check-'))
;(async () => {
  execFileSync(process.execPath, [require.resolve('typescript/bin/tsc'),
    '-p', 'tsconfig.json', '--module', 'commonjs', '--moduleResolution', 'node',
    '--outDir', output], { stdio: 'inherit' })
  const { Notes } = require(join(output, 'index.js'))
  let requests = 0
  const sdk = new Notes({
    baseUrl: 'https://unused.example',
    fetch: async (url, options) => {
      requests += 1
      assert.equal(new URL(url).pathname, '/notes')
      assert.equal(new Headers(options.headers).get('X-SDK-Policy'), 'bundled')
      return new Response(JSON.stringify([{ id: 'note-1', title: 'Bundled policy' }]), {
        status: 200, headers: { 'content-type': 'application/json' },
      })
    },
  })
  // No middleware option: the author's policy runs automatically.
  const notes = await sdk.listNotes()
  assert.equal(notes[0].title, 'Bundled policy')
  assert.equal(requests, 1)
  console.log('SDK author policy runs without consumer registration.')
})().catch(error => {
  console.error(error)
  process.exitCode = 1
}).finally(() => rmSync(output, { recursive: true, force: true }))
