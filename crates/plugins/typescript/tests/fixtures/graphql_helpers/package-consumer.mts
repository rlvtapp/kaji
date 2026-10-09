import { createClient, createGraphqlHttpTransport, zod, faker, msw, reactQuery, vueQuery, swr } from 'poolster-graphql-ecosystem-test';
const transport = createGraphqlHttpTransport('http://example.test/graphql');
const client = createClient({transport});
const result = await client.readUser({id:'42'});
if (result.kind === 'success') {
  const name: string = result.data.user.name;
  // @ts-expect-error ReadUser does not select fragile
  result.data.user.fragile;
}
// @ts-expect-error ID variable must be string
client.readUser({id:42});
zod.ReadUserResultSchema.parse(faker.fakeReadUserResult());
msw.mockReadUser(variables => ({data:{user:{id:variables.id,name:'Ada',nickname:null}}}));
reactQuery.readUserQueryOptions(transport,'public',{id:'42'});
vueQuery.readUserQueryOptions(transport,'public',{id:'42'});
swr.readUserQueryOptions(transport,'public',{id:'42'});
