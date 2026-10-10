# GraphQL collections and command-line clients

These unreleased generators consume a schema plus named operation documents,
using the same native GraphQL contract as the SDKs. Postman exports native GraphQL
requests. Rust and TypeScript CLI packages expose one command per query or mutation.
They do not require an OpenAPI description of the GraphQL endpoint.

## Generate from a recipe

```json
{
  "input": {
    "format": "graphql",
    "provider": "graphql.apollo",
    "path": "schema.graphql",
    "options": { "operation_files": ["operations.graphql"] }
  },
  "output": { "path": "generated" },
  "packages": [
    { "language": "postman", "path": "postman", "plugins": [
      { "name": "collection", "base_url": "http://localhost:4000/graphql" },
      { "name": "environment" }
    ] },
    { "language": "rust-cli", "path": "rust-command", "plugins": [
      { "name": "cli", "command_name": "users", "base_url": "http://localhost:4000/graphql" }
    ] },
    { "language": "typescript-cli", "path": "js-command", "plugins": [
      { "name": "cli", "command_name": "users", "base_url": "http://localhost:4000/graphql" }
    ] }
  ]
}
```

Run `poolster generate --config poolster.json`, then build the generated CLI
package using its README. `--check` verifies regeneration without writing.

## Use a generated command

Given `query ReadUser($id: ID!) { user(id: $id) { id name } }`:

```sh
users read-user --endpoint http://localhost:4000/graphql --variables '{"id":"42"}'
users read-user --variables-file variables.json
```

JSON variables support nested inputs, lists and explicit null; omitted values
stay omitted. Unknown variable names and malformed JSON are rejected. Required
values and GraphQL input coercion are validated by the server. `--variables` and
`--variables-file` are mutually exclusive. Commands send the fixed operation
document and `operationName`; fields cannot be selected dynamically.

Use repeated `--header 'Name: value'` for authentication or other headers. Endpoint
and bearer credentials can be supplied at runtime; generated packages contain no
credentials. Stdout contains the response envelope, preserving partial data, errors
and extensions. Errors produce a nonzero exit code; consult the generated README
for exact codes. Output is JSON, not typed SDK return models. Subscriptions and
incremental delivery are unsupported. OAuth discovery/login generation is not
included for GraphQL.

## Postman

Import `collection.json` and `environment.json`. Requests retain operation names
and native documents, with a JSON variables editor. Environments inherit the configured endpoint and contain illustrative values for
required variables; review and replace those values before running requests.
Optional variables are omitted and operation defaults are applied by the server.
Required opaque or recursive inputs need explicit values through the Rust API
(`.variables(operation, json)`); otherwise strict generation reports an error. Query/mutation grouping can be configured through the Rust API
or the recipe's `group_by_tag` option; this means operation-kind grouping for
GraphQL. The response hook validates envelope structure and records
`poolster_graphql_status` and `poolster_graphql_errors`. GraphQL errors alone do not
fail Newman application assertions; add assertions for your intended outcome.
Operation documents retain literal values, so use variables for sensitive inputs.
Strict mode rejects unsupported operations; `strict: false` can export
the supported subset with diagnostics.

## JavaScript generation

The factories are available from `@relevate/poolster/plugins` in this checkout:

```js
import { generate } from '@relevate/poolster';
import { inputGraphql, pluginGraphqlPostman, pluginGraphqlRustCli,
  pluginGraphqlTypeScriptCli } from '@relevate/poolster/plugins';

await generate({
  input: { path: './schema.graphql', plugin: inputGraphql(), operations: ['./operations.graphql'] },
  output: './generated',
  plugins: [
    pluginGraphqlPostman({ endpoint: 'http://localhost:4000/graphql' }),
    pluginGraphqlRustCli({ commandName: 'users' }),
    pluginGraphqlTypeScriptCli({ commandName: 'users' }),
  ],
});
```

These factories select GraphQL-only tool outputs. Existing HTTP CLI/Postman
generation remains available through the Rust API and CLI recipes.

## Rust plugin API

Use `postman::graphql().input(provider.handle())`,
`rust_cli::graphql().input(provider.handle())`, or
`ts_cli::graphql().input(provider.handle())` with a
`InputProvider<GraphqlOperations>`. Each is added to its existing language package.
For Postman, add `postman::graphql_environment().using_collection(collection.handle())`.
CLI builders accept `.command_name("users")` and `.endpoint("...")`.

[Support matrix](../../plugin-support-matrix.md) · [Reference](../README.md)
