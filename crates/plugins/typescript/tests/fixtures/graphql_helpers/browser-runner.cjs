// Run after graphql_helpers exports a compiled fixture using
// POOLSTER_GRAPHQL_HELPER_FIXTURE_OUTPUT. Dependency paths are explicit pins.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { execFileSync } = require('node:child_process');
const { buildSchema, graphql } = require(process.env.POOLSTER_GRAPHQL_JS);
const cypress = require(path.join(process.env.POOLSTER_CYPRESS_MODULES, 'cypress'));
const project = process.argv[2];
if (!project) throw new Error('Pass the exported GraphQL helper fixture directory');
const schema = buildSchema(fs.readFileSync(path.join(project, 'schema.graphql'), 'utf8'));
const rootValue = {
  user: ({ id }) => ({ id, name: 'Ada', nickname: null }),
  rename: ({ name }) => ({ id: '42', name, nickname: null }),
};
const server = http.createServer(async (request, response) => {
  try {
    if (request.url !== '/graphql') {
      response.setHeader('content-type', 'text/html');
      response.end('<!doctype html><html><body>GraphQL Cypress fixture</body></html>');
      return;
    }
    let body = '';
    for await (const chunk of request) body += chunk;
    const { query: source, operationName, variables: variableValues } = JSON.parse(body);
    const result = await graphql({ schema, source, operationName, variableValues, rootValue });
    response.setHeader('content-type', 'application/json');
    response.end(JSON.stringify(result));
  } catch (error) {
    response.writeHead(500);
    response.end(String(error));
  }
});
(async () => {
  const sdk = path.join(project, 'sdk');
  if (!fs.existsSync(path.join(sdk, 'node_modules'))) {
    fs.symlinkSync(process.env.POOLSTER_GRAPHQL_HELPER_NODE_MODULES, path.join(sdk, 'node_modules'), 'dir');
  }
  execFileSync(process.execPath, [process.env.POOLSTER_TSC_JS, '-p', 'tsconfig.json'], { cwd: sdk, stdio: 'inherit' });
  const specs = path.join(project, 'cypress/e2e');
  fs.mkdirSync(specs, { recursive: true });
  fs.copyFileSync(path.join(__dirname, 'browser.cy.js'), path.join(specs, 'helpers.cy.js'));
  fs.writeFileSync(path.join(project, 'cypress.config.cjs'), 'module.exports = { e2e: { supportFile: false } };\n');
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const results = await cypress.run({
      project,
      browser: 'electron',
      headless: true,
      config: {
        video: false,
        allowCypressEnv: false,
        screenshotOnRunFailure: false,
        e2e: { baseUrl: `http://127.0.0.1:${server.address().port}`, supportFile: false },
      },
    });
    if (results.failures || results.totalFailed !== 0 || results.totalPassed !== 3) {
      console.error(results);
      process.exitCode = 1;
    }
  } finally {
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
