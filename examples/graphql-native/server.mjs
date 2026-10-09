import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { graphql, buildSchema } from 'graphql';

export function createGraphqlServer() {
  const schema = buildSchema(readFileSync(new URL('./schema.graphql', import.meta.url), 'utf8'));
  let name = 'Ada';
  const person = id => ({ id, name, joinedAt: '2026-10-09T00:00:00Z', nickname() { throw new Error('Nickname service unavailable'); } });
  const rootValue = {
    person: ({ id }) => person(id),
    rename: ({ id, name: next }) => { name = next; return person(id); },
  };
  return createServer(async (request, response) => {
    try {
      let body = '';
      for await (const chunk of request) body += chunk;
      const { query, operationName, variables } = JSON.parse(body);
      const result = await graphql({ schema, source: query, operationName, variableValues: variables, rootValue });
      response.setHeader('content-type', 'application/graphql-response+json');
      response.end(JSON.stringify(result));
    } catch (error) {
      response.writeHead(400, { 'content-type': 'application/json' });
      response.end(JSON.stringify({ errors: [{ message: String(error) }] }));
    }
  });
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  createGraphqlServer().listen(4000, '127.0.0.1', () => console.log('GraphQL server: http://127.0.0.1:4000'));
}
