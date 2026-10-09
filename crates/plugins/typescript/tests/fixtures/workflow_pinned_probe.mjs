import assert from 'node:assert/strict';
import http from 'node:http';
import {pathToFileURL} from 'node:url';
const {runWorkflow} = await import(pathToFileURL(process.argv[2]).href);
let called = false;
const server = http.createServer((req,res) => {
  const url = new URL(req.url,'http://localhost');
  assert.equal(req.method,'GET'); assert.equal(url.pathname,'/authorize');
  assert.equal(url.searchParams.get('client_id'),'client');
  assert.equal(url.searchParams.get('redirect_uri'),'https://client.invalid/callback');
  assert.equal(url.searchParams.get('response_type'),'code');
  assert.equal(url.searchParams.get('scope'),'openid');
  assert.equal(url.searchParams.get('state'),'pinned-test');
  called = true; res.setHeader('content-type','application/json'); res.end('{}');
});
await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
try {
  const result = await runWorkflow('authorize',{clientId:'client',redirectUri:'https://client.invalid/callback'},{sourceBaseUrls:{oauth:`http://127.0.0.1:${server.address().port}`}});
  assert.equal(called,true); assert.deepEqual(result.outputs,{status:200});
} finally { await new Promise(resolve => server.close(resolve)); }
