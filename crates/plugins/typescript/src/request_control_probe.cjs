const assert = require('node:assert/strict')
const http = require('node:http')
const { createClient } = require('./runtime.js')
;(async () => {
  let calls = 0, retryCalls = 0, customCalls = 0, closed = false
  const server = http.createServer((req, res) => {
    calls++
    if (req.url === '/custom') { assert.equal(req.method, 'COPY'); customCalls++; res.writeHead(503); res.end(); return }
    if (req.url === '/stall') { req.socket.once('close', () => { closed = true }); return }
    if (req.url === '/retry') { retryCalls++; res.writeHead(503, { 'content-type': 'application/json' }); res.end('{}'); return }
    const finish = () => { res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify({ header: req.headers['x-trace'] })) }
    if (req.url === '/delayed') setTimeout(finish, 50); else finish()
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  try {
    const baseUrl = `http://127.0.0.1:${server.address().port}`
    const config = { baseUrl, headers: { 'X-Trace': 'global' }, retry: { maxAttempts: 3, initialDelayMs: 1000 }, timeoutMs: 1000 }
    if (process.env.KAJI_CONTROL_AXIOS) config.client = require('axios').create({ baseURL: baseUrl, proxy: false })
    const client = createClient(config)
    await assert.rejects(client({ method: 'COPY', url: '/custom' }), error => error.status === 503); assert.equal(customCalls, 1)
    const options = { headers: { 'X-Trace': 'call' }, timeoutMs: 200 }
    const response = await client({ method: 'GET', url: '/headers', headers: { 'X-Trace': 'declared' }, requestOptions: options })
    assert.equal(response.data.header, 'call'); assert.deepEqual(options, { headers: { 'X-Trace': 'call' }, timeoutMs: 200 })
    const rewriting = createClient({ ...config, middleware: [(request, next) => { request.requestOptions.headers.set('X-Trace', 'middleware'); return next(request) }] })
    const rewritten = await rewriting({ method: 'GET', url: '/headers', requestOptions: options })
    assert.equal(rewritten.data.header, 'middleware'); assert.equal(options.headers['X-Trace'], 'call')
    const invalidCount = calls
    await assert.rejects(client({ method: 'GET', url: '/headers', requestOptions: { timeoutMs: 0 } }), RangeError)
    assert.equal(calls, invalidCount)
    const start = Date.now()
    await assert.rejects(client({ method: 'GET', url: '/stall', requestOptions: { timeoutMs: 30 } }), error => error.name === 'TimeoutError')
    assert.ok(Date.now() - start < 400)
    await new Promise(resolve => setTimeout(resolve, 30)); assert.ok(closed, 'native driver did not close cancelled socket')
    await assert.rejects(client({ method: 'GET', url: '/retry', requestOptions: { timeoutMs: 30 } }), error => error.name === 'TimeoutError')
    await new Promise(resolve => setTimeout(resolve, 30)); assert.equal(retryCalls, 1)
    const cancel = new AbortController(); cancel.abort()
    const count = calls
    await assert.rejects(client({ method: 'GET', url: '/headers', requestOptions: { signal: cancel.signal } }), error => error.name === 'AbortError')
    assert.equal(calls, count)
    const [short, long] = await Promise.all([
      client({ method: 'GET', url: '/delayed', requestOptions: { timeoutMs: 10 } }).catch(error => error.name),
      client({ method: 'GET', url: '/delayed', requestOptions: { timeoutMs: 300 } })
    ])
    assert.equal(short, 'TimeoutError'); assert.equal(long.data.header, 'global')
    const blocked = createClient({ ...config, middleware: [async (request, next) => { await new Promise(resolve => setTimeout(resolve, 80)); return next(request) }] })
    const before = calls
    await assert.rejects(blocked({ method: 'GET', url: '/headers', requestOptions: { timeoutMs: 10 } }), error => error.name === 'TimeoutError')
    await new Promise(resolve => setTimeout(resolve, 100)); assert.equal(calls, before, 'late middleware reached driver after deadline')
  } finally { await new Promise(resolve => server.close(resolve)) }
})().catch(error => { console.error(error); process.exitCode = 1 })
