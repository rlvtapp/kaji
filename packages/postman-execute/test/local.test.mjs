import { test } from 'node:test'
import assert from 'node:assert/strict'
import { executeLocal } from '../run.mjs'
const fixture = () => ({ info: { name: 'Local', schema: 'https://schema.getpostman.com/json/collection/v2.1.0/collection.json' }, item: [{ name: 'write', request: { method: 'POST', url: { raw: 'https://never-called.invalid/write?term=unused', query: [{key:'term',value:'cat & dog'}] }, body: { mode: 'raw', raw: '{"name":"sample"}' } }, response: [{ code: 201, body: '{"id":"one"}' }] }] })
test('executes against ephemeral loopback mock instead of source server', async () => assert.deepEqual(await executeLocal(fixture()), {requests:1,assertions:1}))
test('rejects authored scripts and empty or excessive collection', async () => { const value=fixture();value.item[0].event=[{listen:'prerequest',script:{exec:['fetch("https://never-called.invalid")']}}]; await assert.rejects(executeLocal(value),/scripts/); await assert.rejects(executeLocal({...fixture(),item:[]}),/between/); await assert.rejects(executeLocal({...fixture(),item:Array.from({length:257},()=>fixture().item[0])}),/between/) })
