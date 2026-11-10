import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {expenseOracle,parseReport,createRunDirectory} from './usability/oracle.mjs';

test('expense oracle keeps exact signed bounds and rejects overflow without committing a row',()=>{
 const rows=['{"category":"x","amount":9223372036854775807}','{"category":"x","amount":1}','{"category":"y","amount":-9223372036854775808}','{"category":"y","amount":-1}'];
 assert.deepEqual(expenseOracle(rows),{accepted:2n,rejected:2n,totals:{x:9223372036854775807n,y:-9223372036854775808n}});
 assert.deepEqual(parseReport('{"accepted":2,"rejected":2,"totals":{"x":9223372036854775807,"y":-9223372036854775808}}'),expenseOracle(rows));
});
test('oracle distinguishes malformed/wrong-type/fractional rows and the changed policy',()=>{
 const rows=['null','[]','{','{"category":2,"amount":3}','{"category":"x","amount":"3"}','{"category":"x","amount":1.5}','{"category":"x","amount":-3}','{"category":"__proto__","amount":2}'];
 assert.deepEqual(expenseOracle(rows),{accepted:2n,rejected:6n,totals:Object.fromEntries([['x',-3n],['__proto__',2n]])});
 assert.deepEqual(expenseOracle(rows,true),{accepted:1n,rejected:7n,totals:Object.fromEntries([['__proto__',2n]])});
});
test('evaluation IDs cannot escape or overwrite an existing run',async()=>{
 const root=await mkdtemp(join(tmpdir(),'walz-usability-'));
 try{
  for(const id of ['..','../x','/tmp/x','a/b',''])await assert.rejects(createRunDirectory(root,id));
  assert.equal(await createRunDirectory(root,'trial-1'),join(root,'trial-1'));
  await assert.rejects(createRunDirectory(root,'trial-1'));
 }finally{await rm(root,{recursive:true,force:true});}
});
