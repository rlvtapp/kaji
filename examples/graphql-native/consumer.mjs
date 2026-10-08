import assert from 'node:assert/strict';
import { PersonName, RenamePerson, PersonWithNickname, createGraphqlHttpTransport } from './generated/client/dist/index.js';

export async function runConsumer(endpoint) {
  const transport = createGraphqlHttpTransport(endpoint);
  const first = await PersonName(transport, { id: '1' });
  assert.equal(first.kind, 'success');
  console.log('Query:', first.data.person);

  const renamed = await RenamePerson(transport, { id: '1', name: 'Grace' });
  assert.equal(renamed.kind, 'success');
  assert.equal(renamed.data.rename.name, 'Grace');
  console.log('Mutation:', renamed.data.rename);

  const result = await PersonWithNickname(transport, { id: '1' });
  // GraphQL application errors do not discard usable selected data.
  switch (result.kind) {
    case 'success': console.log('Complete data:', result.data); break;
    case 'partial':
      assert.equal(result.data.person.name, 'Grace');
      assert.equal(result.data.person.nickname, null);
      console.log('Partial data:', result.data);
      console.log('GraphQL errors:', result.errors.map(error => error.message));
      break;
    case 'error': throw new Error(result.errors.map(error => error.message).join('; '));
  }
  assert.equal(result.kind, 'partial');
}
