import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {oracle,parseReport,createRunDirectory,scalingSource} from './macro-usability/support.mjs';

test('macro trial oracle preserves exact integer bounds, duplicate keys and transactional overflow',()=>{
  const lines=['{"category":"a","amount":9223372036854775807}',
    '{"category":"a","amount":1}', '{"category":"b","amount":-9223372036854775808}',
    '{"category":"b","amount":-1}', '{"category":"__proto__","amount":2,"amount":3}',
    '{"category":"bad","amount":1e0}', '{"category":"bad","amount":1.0}',
    '{"category":"bad","amount":"3"}', 'null'];
  assert.deepEqual(oracle(lines),{accepted:3n,rejected:6n,totals:Object.fromEntries([['a',9223372036854775807n],['b',-9223372036854775808n],['__proto__',3n]])});
  assert.deepEqual(parseReport('{"accepted":1,"rejected":0,"totals":{"x":9223372036854775807}}'),
    {accepted:1n,rejected:0n,totals:{x:9223372036854775807n}});
  assert.deepEqual(oracle(['{"category":"x","amount":-1}'],true),{accepted:0n,rejected:1n,totals:{}});
});
test('macro trial does not overwrite prior runs or escape the run root',async()=>{
  const root=await mkdtemp(join(tmpdir(),'wlt-macro-eval-'));
  try{
    for(const id of ['','..','../x','/tmp/x','a/b'])await assert.rejects(createRunDirectory(root,id));
    assert.equal(await createRunDirectory(root,'round-1'),join(root,'round-1'));
    await assert.rejects(createRunDirectory(root,'round-1'));
  }finally{await rm(root,{recursive:true,force:true});}
});
test('scale inputs change the number of equivalent real call sites',()=>{
  for(const n of [1,8,128]){
    for(const macros of [false,true]){
      const source=scalingSource(n,macros);
      assert.equal((source.match(/\(func \$f\d+/g)??[]).length,n);
      assert.equal((source.match(/\(%\$double \$x\)/g)??[]).length,macros?n:0);
    }
  }
});
