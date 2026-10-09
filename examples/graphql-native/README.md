# Native GraphQL → TypeScript

Poolster validates a schema together with operation documents and generates a package whose result types contain only each operation's selections. This example uses queries, a mutation and an intentional resolver failure to show usable partial data with explicit GraphQL errors.

The recipe selects `contracts.graphql.style: "flat"`. The JavaScript consumer
configures its endpoint once with `createClient({ endpoint })`, then calls
`client.personName({ id: '1' })` and `client.renamePerson(...)`. Method names come
from the named operation documents; selections remain fixed by those documents.

With Node.js 22 or newer and the Poolster CLI on your PATH, run from this directory:

```sh
npm install
npm run generate
npm run build
npm run demo
```

The demo starts a local GraphQL server on a temporary port, executes the generated client, checks the results, and closes the server. Dependencies are pinned to GraphQL 16.14.2 and TypeScript 5.9.3. Generated files are ignored by Git.

To use a checkout instead of an installed CLI, from the repository root:

```sh
cargo build -p poolster-cli
cd examples/graphql-native
npm install
../../target/debug/poolster generate --config poolster.json
npm run build
npm run demo
```

Expected output includes the query result for Ada, the mutation result for Grace, partial data with `nickname: null`, and `Nickname service unavailable`. HTTP failures throw `GraphqlHttpError`; malformed response envelopes throw `GraphqlProtocolError`. GraphQL application errors return `success`, `partial`, or `error` envelopes.

`npm run server` also starts the same server at http://127.0.0.1:4000 for manual experiments. This example exercises HTTP query and mutation generation. Network subscriptions require a separately supplied transport and are outside this example.

The recipe maps the custom `DateTime` scalar to `string` for both input and
output. `PersonName` selects `joinedAt`, and the consumer checks its JSON string
value. Mappings describe the wire representation; they do not parse dates or
serialize application objects. Unconfigured custom scalars stay `unknown`.
