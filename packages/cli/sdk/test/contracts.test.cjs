'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const { defineContract, providerHandle, requireContract, generate } = require('../index.cjs');
const { artifacts, config, temporary } = require('../test-support/helpers.cjs');

test('JavaScript provider contracts and phases mirror the native graph rules', async (t) => {
  const compiled = await artifacts(t);
  const dir = await temporary(t);
  const Symbols = defineContract('symbols');
  const Missing = defineContract('missing');

  await t.test('a post consumer reads a selected provider and shared workspace', async () => {
    const events = [];
    const first = { name: 'first', provides: [Symbols], generate(ctx) {
      events.push('first');
      ctx.workspace.set('marker', 'shared');
      ctx.publish(Symbols, ['alpha']);
    } };
    const second = { name: 'second', provides: [Symbols], generate(ctx) {
      events.push('second');
      ctx.publish(Symbols, ['beta']);
    } };
    const consumer = {
      name: 'consumer', phase: 'post',
      requires: [requireContract(Symbols, { from: providerHandle(second, Symbols) }), requireContract(Missing, { optional: true })],
      generate(ctx) {
        events.push('consumer');
        assert.deepEqual(ctx.inputs.get(Symbols), ['beta']);
        assert.equal(ctx.inputs.optional(Missing), undefined);
        assert.equal(ctx.workspace.get('marker'), 'shared');
        ctx.emitFile({ path: 'selected.txt', contents: ctx.inputs.get(Symbols).join(',') });
      },
    };
    const result = await generate(config({ artifacts: compiled }, path.join(dir, 'selected'), [consumer, first, second]), { write: false });
    assert.deepEqual(events, ['second', 'first', 'consumer']);
    assert.equal(result.files[0].contents, 'beta');
  });

  await t.test('ambiguous, missing, and invalid phase dependencies reject before output', async () => {
    const a = { name: 'a', provides: [Symbols], generate(ctx) { ctx.publish(Symbols, 1); } };
    const b = { name: 'b', provides: [Symbols], generate(ctx) { ctx.publish(Symbols, 2); } };
    const ambiguous = { name: 'ambiguous', requires: [requireContract(Symbols)], generate() {} };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'ambiguous'), [a, b, ambiguous])), /Select a provider handle/);
    const absent = { name: 'absent', requires: [requireContract(Missing)], generate() {} };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'absent'), [absent])), /no provider is registered/);
    const post = { name: 'post', phase: 'post', provides: [Symbols], generate(ctx) { ctx.publish(Symbols, 1); } };
    const early = { name: 'early', requires: [requireContract(Symbols)], generate() {} };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'phase'), [early, post])), /cannot depend on post/);
    const wrong = { name: 'wrong', requires: [requireContract(Symbols, { from: providerHandle(a, Symbols) })], generate() {} };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'handle'), [wrong, b])), /unregistered provider handle/);
  });

  await t.test('undeclared reads, missing publications, and duplicate publications reject', async () => {
    const read = { name: 'read', generate(ctx) { ctx.inputs.get(Symbols); } };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'read'), [read])), /undeclared contract read/);
    const missingPublish = { name: 'missing-publish', provides: [Symbols], generate() {} };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'publish'), [missingPublish])), /did not publish declared contract/);
    const duplicate = { name: 'duplicate', provides: [Symbols], generate(ctx) { ctx.publish(Symbols, 1); ctx.publish(Symbols, 2); } };
    await assert.rejects(generate(config({ artifacts: compiled }, path.join(dir, 'duplicate'), [duplicate])), /published symbols twice/);
  });
});
