import assert from 'node:assert/strict';
import { createClient } from './generated/client/dist/index.js';

export async function runConsumer(endpoint) {
  const client = createClient({ endpoint });
  const first = await client.personName({ id: '1' });
  assert.equal(first.kind, 'success');
  assert.equal(first.data.person.joinedAt, '2026-10-09T00:00:00Z');
  console.log('Query:', first.data.person);

  const renamed = await client.renamePerson({ id: '1', name: 'Grace' });
  assert.equal(renamed.kind, 'success');
  assert.equal(renamed.data.rename.name, 'Grace');
  console.log('Mutation:', renamed.data.rename);

  const result = await client.personWithNickname({ id: '1' });
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
