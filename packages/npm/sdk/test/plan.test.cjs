const test = require('node:test');
const assert = require('node:assert/strict');
const { plan, formatPlan, createPoolster, defineContract, providerHandle, requireContract } = require('../index.cjs');
const Data = defineContract('test.data');
const config = plugins => ({ name: 'test', version: '1', input: './does-not-exist.yaml', output: './generated', plugins });
test('SDK plan selects dependencies without reading inputs or running plugins', () => {
  const producer = { name: 'producer', provides: [Data], generate() { throw Error('must not run'); } };
  const consumer = { name: 'consumer', requires: [requireContract(Data, { from: providerHandle(producer, Data) })], generate() { throw Error('must not run'); } };
  const result = createPoolster(config([consumer, producer])).plan();
  assert.equal(result.plugins[1].order, 0);
  assert.deepEqual(result.edges[0], { provider: 1, consumer: 0, contract: 'test.data', dependency: null, optional: false, selection: 'explicit' });
  assert.match(formatPlan(result), /test.data from #1/);
  assert.deepEqual(JSON.parse(JSON.stringify(result)), result);
});
test('SDK planning uses graph ambiguity checks and optional absence', () => {
  const a = { name: 'a', provides: [Data], generate() {} };
  const b = { name: 'b', provides: [Data], generate() {} };
  const consumer = { name: 'consumer', requires: [requireContract(Data)], generate() {} };
  assert.throws(() => plan(config([a,b,consumer])), /Select a provider/);
  consumer.requires = [requireContract(Data, { optional: true })];
  assert.equal(plan(config([consumer])).edges[0].provider, null);
});
test('native factory instances remain visible as distinct opaque package boundaries', () => {
  const native = path => ({ name: 'native-typescript', kind: 'native-sdk', package: { language: 'typescript', path } });
  const result = plan(config([native('a'), native('b')]));
  assert.equal(result.plugins.length, 2);
  assert.equal(result.plugins[0].detail, 'opaque-native-configuration');
  assert.equal(result.plugins[1].id, 1);
});
