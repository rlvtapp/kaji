import { createClient, createGraphqlSseTransport, createGraphqlHttpTransport, type GraphqlScalarCodecs, Read } from './index.js';
const codecs: GraphqlScalarCodecs = { DateTime: { encode: (value) => value.toISOString(), decode: (value) => new Date(String(value)) } };
const client = createClient({ endpoint: 'http://localhost/graphql', scalarCodecs: codecs, subscriptionTransport: createGraphqlSseTransport('http://localhost/graphql') });
async function checked() {
  const result = await client.read({ input: { when: new Date(), list: [null, new Date()], next: { when: new Date() } }, include: false });
  if (result.kind !== 'error') { const date: Date = result.data.stamp.when; date.toISOString(); }
  for await (const result of client.ticks({ start: new Date() })) if (result.kind !== 'error') result.data.ticks.when.toISOString();
  await Read(createGraphqlHttpTransport('http://localhost/graphql', { scalarCodecs: codecs }), { input: { when: new Date() }, include: true });
}
void checked;
