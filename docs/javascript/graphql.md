# Generate a GraphQL client

← [JavaScript](README.md)

Supply a complete schema and named operation documents. These additions are
unreleased; use a build from this checkout.

```graphql
query ReadUser($id: ID!) {
  user(id: $id) { id name }
}
```

```js
import { defineConfig } from '@relevate/poolster';
import { inputGraphql } from '@relevate/poolster-input-graphql';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';

export default defineConfig({
  input: {
    path: './schema.graphql',
    plugin: inputGraphql(),
    operations: ['./operations.graphql'],
  },
  output: './generated',
  plugins: [pluginTypeScript({
    path: 'client',
    contracts: { graphql: { style: 'flat' } },
  })],
});
```

After generation and compilation:

```js
import { createClient } from './generated/client/dist/index.js';

const client = createClient({ endpoint: 'https://example.test/graphql' });
const result = await client.readUser({ id: '42' });
if (result.kind === 'success') console.log(result.data.user);
if (result.kind === 'partial') console.log(result.data, result.errors);
if (result.kind === 'error') console.error(result.errors);
```

## Choose a style

| Style | Call |
| --- | --- |
| `flat` | `client.readUser(variables)` |
| `grouped` | `client.query.readUser(variables)` |
| `raw` | `readUser(transport, variables)` |

Explicit groups can replace the operation-kind group:

```js
contracts: {
  graphql: {
    style: 'grouped',
    groups: { user: { read: 'ReadUser', rename: 'RenameUser' } },
  },
}
// client.user.read(...) and client.user.rename(...)
```

Selections come from operation documents. A dynamic `fields` parameter is
[planned](../proposals/graphql-selection-builder-proposal.md), not implemented.

**Next:** [Scalar mappings and transport](../reference/outputs/graphql-typescript.md) ·
[Query hooks and mocks](../reference/outputs/graphql-integrations.md) · [Runnable example](../../examples/graphql-native/README.md)

The existing Go and Python plugin factories also accept GraphQL input:
[Go client](../reference/outputs/graphql-go.md) · [Python client](../reference/outputs/graphql-python.md).
