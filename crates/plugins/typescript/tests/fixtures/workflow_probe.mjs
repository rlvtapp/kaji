import assert from 'node:assert/strict';
import http from 'node:http';
import {pathToFileURL} from 'node:url';
const {runWorkflow, runWorkflow_636865636b6f7574: checkout, WorkflowExecutionError} = await import(pathToFileURL(process.argv[2]).href);
const seen = [];
const server = http.createServer(async (req, res) => {
  let text = ''; for await (const chunk of req) text += chunk;
  seen.push({url:req.url, headers:req.headers, body:text});
  res.setHeader('content-type', 'application/json');
  if(req.url === '/api/session') { assert.deepEqual(JSON.parse(text), {username:'guest'}); res.end(JSON.stringify({token:'session-token'})); }
  else {
    assert.equal(req.headers['x-session'], 'session-token');
    if(req.url.startsWith('/api/orders/fail?')) { res.statusCode = 500; res.end(JSON.stringify({error:'no order'})); }
    else if(req.url.startsWith('/api/orders/malformed?')) { res.statusCode = 201; res.end('{bad'); }
    else { assert.equal(req.url, '/api/orders/item%2F1?quantity=1'); res.statusCode = 201; res.end(JSON.stringify({id:'order-1'})); }
  }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const options = {sourceBaseUrls:{shop:`http://127.0.0.1:${server.address().port}/api`}};
try {
  const result = await checkout({itemId:'item/1'}, options);
  assert.deepEqual(result.outputs,{orderId:'order-1'});
  assert.equal(result.dependencies.login.outputs.token, 'session-token');
  assert.equal(result.steps.create.status,201);
  assert.equal(seen.length,2);
  await assert.rejects(checkout({itemId:'fail'},options), error => {
    assert(error instanceof WorkflowExecutionError);
    assert.equal(error.stepId,'create');
    assert.equal(error.execution.steps.create.status,500);
    assert.deepEqual(error.execution.steps.create.body,{error:'no order'});
    assert.equal(error.execution.dependencies.login.outputs.token,'session-token');
    return true;
  });
  await assert.rejects(checkout({itemId:'malformed'},options), error => error instanceof WorkflowExecutionError && error.cause.message.includes('not JSON'));
  const count = seen.length;
  await assert.rejects(checkout({itemId:'1',quantity:'bad'},options), error => error.cause.message.includes('Invalid workflow input'));
  assert.equal(seen.length,count);
  const controller = new AbortController(); controller.abort();
  await assert.rejects(checkout({itemId:'1'},{...options,signal:controller.signal}), WorkflowExecutionError);
  assert.equal(seen.length,count);
  await assert.rejects(runWorkflow('missing',{},options), /Unknown workflow/);
  console.log('workflow runtime: dependencies, defaults, HTTP path/query/headers, outputs, failure partial state, malformed inputs, cancellation passed');
} finally { await new Promise(resolve => server.close(resolve)); }
