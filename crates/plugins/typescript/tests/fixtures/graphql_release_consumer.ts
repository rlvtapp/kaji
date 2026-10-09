import { Read, Partial, Rename, Fatal, createGraphqlHttpTransport, releaseMarker, type ReadVariables } from '@poolster-test/graphql-client';
const transport = createGraphqlHttpTransport('http://localhost');
const variables: ReadVariables = { id: '7', filter: { prefix: 'selected' } };
async function check(): Promise<void> {
  const result = await Read(transport, variables);
  if (result.kind !== 'error') {
    const name: string = result.data.person.name;
    const nickname: string | null = result.data.person.nickname;
    // @ts-expect-error nullable selected field cannot be treated as non-null
    const unsafeNickname: string = result.data.person.nickname;
    // @ts-expect-error unselected fields are absent from this operation result
    result.data.person.fragile;
    void [name, nickname, unsafeNickname];
  }
  // @ts-expect-error required operation variable
  await Read(transport, {});
  // @ts-expect-error required nested input object property
  await Read(transport, { id: '7', filter: {} });
  // @ts-expect-error mutation variables retain scalar types
  await Rename(transport, { name: 3 });
  await Partial(transport, { id: '7' });
  await Fatal(transport, {});
  const marker: string = releaseMarker;
  void marker;
}
void check;
