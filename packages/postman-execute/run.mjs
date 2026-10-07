import http from 'node:http'
import fs from 'node:fs'
import newman from 'newman'
import { pathToFileURL } from 'node:url'
import path from 'node:path'

export async function executeLocal(original) {
  const collection = structuredClone(original)
  const requests = []
  function visit(value) {
    if (!value || typeof value !== 'object') return
    if (Array.isArray(value)) { value.forEach(visit); return }
    if (value.event?.length) throw new Error('Collection scripts are not accepted by this mock runner')
    if (value.request) requests.push(value)
    Object.values(value).forEach(visit)
  }
  visit(collection)
  if (!requests.length || requests.length > 256) throw new Error('Expected between 1 and 256 requests')
  const fixtures = requests.map((item, index) => {
    const response = (item.response ?? []).find(value => value.code >= 200 && value.code < 300)
    const code = response?.code ?? 200
    return { method: item.request.method, query: (typeof item.request.url === 'object' ? item.request.url.query ?? [] : []).filter(q => !q.disabled && !String(q.value).includes('{{')), code, body: response?.body ?? '', contentType: response?.header?.find(h => h.key.toLowerCase() === 'content-type')?.value ?? 'application/json', index }
  })
  let received = 0
  const server = http.createServer((request, response) => {
    let bytes = 0
    request.on('data', chunk => { bytes += chunk.length; if (bytes > 1024 * 1024) request.destroy() })
    request.on('end', () => {
      const match = /^\/fixture\/(\d+)$/.exec(request.url.split('?')[0])
      const fixture = match && fixtures[Number(match[1])]
      if (!fixture) { response.writeHead(404); response.end(); return }
      const url = new URL(request.url, 'http://127.0.0.1')
      if (request.method !== fixture.method || fixture.query.some(q => !url.searchParams.getAll(q.key).includes(String(q.value)))) { response.writeHead(400); response.end(); return }
      received++
      response.writeHead(fixture.code, { 'content-type': fixture.contentType })
      response.end(fixture.body)
    })
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  const base = `http://127.0.0.1:${server.address().port}`
  try {
    collection.auth = { type: 'noauth' }
    for (const [index, item] of requests.entries()) {
      const query = typeof item.request.url === 'object' ? item.request.url.query ?? [] : []
      item.request.url = { raw: `${base}/fixture/${index}`, host: ['127.0.0.1'], port: String(server.address().port), protocol: 'http', path: ['fixture', String(index)], query }
      item.request.auth = { type: 'noauth' }
      item.event = [{ listen: 'test', script: { type: 'text/javascript', exec: [`pm.test('mock status', () => pm.expect(pm.response.code).to.eql(${fixtures[index].code}))`] } }]
    }
    const summary = await new Promise((resolve, reject) => newman.run({ collection, timeout: 15000, timeoutRequest: 1500, timeoutScript: 500, iterationCount: 1, ignoreRedirects: true, reporters: [] }, (error, result) => error ? reject(new Error('Local collection execution failed')) : resolve(result)))
    if (summary.run.failures.length || received !== requests.length) throw new Error(`Local collection execution failed (${summary.run.failures.length} failures; ${received}/${requests.length} requests)`)
    return { requests: received, assertions: summary.run.stats.assertions.total }
  } finally { await new Promise(resolve => server.close(resolve)) }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const file = process.argv[2]
    if (process.env.KAJI_ACTION_PATH && path.isAbsolute(file ?? '')) throw new Error('Action collection path must be checkout relative')
    if (!file || file.split(/[\\/]/).includes('..')) throw new Error('Invalid collection path')
    const resolved=fs.realpathSync(file)
    if (!path.isAbsolute(file) && !resolved.startsWith(fs.realpathSync(process.cwd()) + path.sep)) throw new Error('Collection must remain within checkout')
    if (fs.statSync(resolved).size > 10 * 1024 * 1024) throw new Error('Collection exceeds 10 MiB')
    console.log(JSON.stringify(await executeLocal(JSON.parse(fs.readFileSync(resolved, 'utf8')))))
  }
  catch (error) { console.error(error.message); process.exitCode = 1 }
}
