# GraphQL JavaScript integrations

These integrations consume the selected GraphQL client contract, including its
actual emitted symbols, operation definitions and scalar mappings. They do not
convert GraphQL operations into HTTP endpoints. Fixed operation documents determine
variables and selected results in raw, flat and grouped client styles alike.

This work is unreleased. Consult the [support matrix](plugin-support-matrix.md)
and [verification record](verification.md) for completed checks and limitations.

## Compose a package

An input recipe can select the GraphQL SDK together with its companions:

```json
{
  "input": {
    "format": "graphql",
    "path": "schema.graphql",
    "options": { "operation_files": ["operations.graphql"] }
  },
  "output": { "path": "generated" },
  "packages": [{
    "language": "typescript",
    "path": "client",
    "plugins": [
      { "name": "graphql", "contracts": { "graphql": { "style": "flat" } } },
      { "name": "react-query" },
      { "name": "vue-query" },
      { "name": "swr" },
      { "name": "zod" },
      { "name": "faker" },
      { "name": "msw" },
      { "name": "cypress", "cypress_options": { "include_mutations": true } }
    ]
  }]
}
```

Choose only the frameworks and helpers needed by your application. Companions
are exported under namespaces such as `reactQuery`, `vueQuery`, `swr`, `zod`,
`faker` and `msw` to avoid collisions between helper names. Cypress is a separate
module so importing the main SDK does not evaluate browser-test globals.

In the Rust plugin API, bind companions explicitly to the client provider:

```rust
let client = ts::graphql(Some(input.handle())).flat();
let provider = client.handle();
let package = ts::package("client")
    .with(ts::react_query().using_graphql(Some(provider)))
    .with(ts::zod().using_graphql(Some(provider)))
    .with(client)
    .with(input);
```

The dependency graph binds helpers to the selected client regardless of plugin
registration order. Explicit provider handles also prevent accidentally mixing
metadata from different clients.

## Queries, mutations and caching

For an operation named `ReadUser`, the React companion emits
`reactQuery.useReadUser(...)` and `reactQuery.readUserQueryOptions(...)`. Vue Query
and SWR expose equivalent operation-based helpers in their own namespaces.

```js
import { createClient, createGraphqlHttpTransport, reactQuery } from './client/dist/index.js';

const transport = createGraphqlHttpTransport('https://example.test/graphql');
const client = createClient({ transport });
// Within a component with a configured QueryClientProvider:
const query = reactQuery.useReadUser(transport, 'example/public', { id: '42' });
```

Cache keys include the operation, variables, transport identity and explicit
`cacheScope`. The scope should identify the endpoint and authentication context;
change it when that context changes. Keep the transport instance stable across
renders. Group mappings affect the SDK facade, while helper names remain tied to
named operations.

Hooks retain the complete `GraphqlResult` success/partial/error union. A query
library's successful fetch does not imply GraphQL application success: inspect
`query.data.kind` before using data. Transport failures use the framework's error
path. TanStack query cancellation forwards the query's AbortSignal; mutations
disable automatic retry. Subscription operations require a separate transport
and are outside these query helpers.

Vue query hooks accept reactive variables and cache scope through Vue refs or
getters; options update when either changes. Query-options utilities accept plain
values. SWR accepts a caller-supplied signal in request options. Helpers currently
emit a single file; infinite-pagination helpers are not supported.

## Validation, fixtures and mocks

Zod generates variables and selected data schemas, for example
`zod.ReadUserVariablesSchema` and `zod.ReadUserResultSchema`. These validate the
data shape rather than treating every response as successful. Nullable fields
remain distinct from optional fields; required nullable fields must be present.

Faker generates functions such as `faker.fakeReadUserVariables()` and
`faker.fakeReadUserResult()`. Fixtures follow the selected shape, including enums,
nullability and bounded recursive input generation. They are synthetic values,
not evidence that the server accepts a particular business value.

MSW generates query/mutation handlers using GraphQL operation names:

```js
const handler = msw.mockReadUser(variables => ({
  data: { user: { id: variables.id, name: 'Ada', nickname: null } },
}));
```

Handlers can return GraphQL data with errors to exercise partial results, or
errors without data. Unrelated operations remain available to other handlers or
the network according to your MSW configuration.

Cypress generates `requestReadUser(variables, { url })` and
`interceptReadUser(envelope, url)`. Interception checks `operationName` and aliases
only matching requests. Query helpers are enabled by default; mutation helpers
require the Cypress `include_mutations` option. They use actual GraphQL POST
documents and variables rather than inventing REST routes.

Runtime validation and fixture generation currently support primitive custom
scalar wire mappings. Complex mapped types need explicit runtime handling;
generation rejects unsupported mapped expressions rather than inventing codecs.
Existing HTTP-specific options do not automatically apply to GraphQL helpers.
See each generator's compatibility diagnostics for unsupported options.

Dynamic caller-selected `fields` remain a [planned feature](graphql-selection-builder-proposal.md).
