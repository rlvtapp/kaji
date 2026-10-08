import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, symlinkSync, rmSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { packageDirectory, plan, execute } from '../check.mjs';

test('plugin commands stay literal and failing checks stop publication', () => {
  const root = mkdtempSync(join(tmpdir(), 'poolster-check-'));
  try {
    mkdirSync(join(root, '.poolster'));
    writeFileSync(join(root, '.poolster/package.json'), JSON.stringify({schema_version:1,language:'community',build:[{program:'builder',args:['$(literal)']}],test:[{program:'tester'}]}));
    const calls = [];
    execute(plan(root, 'community'), root, (program,args,options) => { calls.push({program,args,options}); return {status:0}; });
    assert.equal(calls[0].args[0], '$(literal)');
    assert.equal(calls[0].options.shell, false);
    assert.throws(() => execute(plan(root,'community'),root,()=>({status:1})), /failed/);
    assert.throws(() => plan(root,'typescript'), /mismatch/);
  } finally { rmSync(root,{recursive:true,force:true}); }
});
test('paths cannot escape through traversal or symlink', () => {
  const root = mkdtempSync(join(tmpdir(),'poolster-root-'));
  const outside = mkdtempSync(join(tmpdir(),'poolster-outside-'));
  try {
    assert.equal(packageDirectory(root,'.'),realpathSync(root));
    assert.throws(()=>packageDirectory(root,outside),/relative/);
    symlinkSync(outside,join(root,'escaped'),'dir');
    assert.throws(()=>packageDirectory(root,'escaped'),/escapes/);
  } finally { rmSync(root,{recursive:true,force:true});rmSync(outside,{recursive:true,force:true}); }
});
test('native defaults select real tools and exclude dependency directories', () => {
  const root = mkdtempSync(join(tmpdir(),'poolster-native-'));
  try {
    mkdirSync(join(root,'src'));mkdirSync(join(root,'vendor'));
    writeFileSync(join(root,'src','Model.php'),'<?php');writeFileSync(join(root,'vendor','Other.php'),'<?php');
    assert.deepEqual(plan(root,'php'),[{program:'php',args:['-l','src/Model.php']}]);
    assert.equal(plan(root,'rust')[0].program,'cargo');
    assert.equal(plan(root,'go')[0].program,'go');
    assert.deepEqual(plan(root,'terraform'), [{program:'go',args:['mod','tidy']},{program:'go',args:['test','./...']}]);
    assert.throws(()=>plan(root,'custom'),/metadata/);
  } finally { rmSync(root,{recursive:true,force:true}); }
});
