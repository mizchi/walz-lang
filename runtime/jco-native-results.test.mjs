import test from 'node:test';
import assert from 'node:assert/strict';
import {nativeHostResults} from './jco-native-results.mjs';

const fixture=provided=>`const _trampoline0=async function(){const hostProvided=${provided};let ret;
  try{ret=await host();}catch(err){throw err;}
  const encoded=await((...args)=>realloc0Async(...args))(0,0,1,8);
  if(hostProvided){task.resolve([ret]);task.exit();return ret;}
  return encoded;
};`;
test('only the unused async host result copy is retired, while guest result storage stays live',async()=>{
  for(const provided of [true,false]) {
    const source=nativeHostResults(fixture(provided)),live=new Set();let next=8,resolved=0;
    const realloc=async(ptr,oldSize,align,size)=>{if(ptr)assert.ok(live.delete(ptr));if(!size)return 0;const out=next;next+=size;live.add(out);return out;};
    const run=Function('host','realloc0Async','task',`${source};return _trampoline0();`);
    const result=await run(()=>Promise.resolve('value'),realloc,{resolve(v){resolved++;assert.equal(live.size,0,'unused result retired before canonical result lowering');assert.deepEqual(v,['value']);},exit(){}});
    assert.equal(resolved,provided?1:0);assert.equal(live.size,provided?0:1);
    if(!provided)await realloc(result,8,1,0);
    assert.equal(live.size,0);assert.equal(nativeHostResults(source),source);
  }
});
test('host failure does not allocate or complete a result',async()=>{
  let touched=false;
  const run=Function('host','realloc0Async','task',`${nativeHostResults(fixture(true))};return _trampoline0();`);
  await assert.rejects(run(()=>Promise.reject(new Error('failed')),()=>{touched=true;},{resolve(){touched=true;},exit(){touched=true;}}),/failed/);
  assert.equal(touched,false);
});
test('a rejected native host task propagates instead of returning an unsettled completion promise',async()=>{
  const source=fixture(true).replace('catch(err){throw err;}','catch(err){task.reject(err);task.exit();return task.completionPromise();}');
  let rejected=0,exited=0;
  const run=Function('host','realloc0Async','task',`${nativeHostResults(source)};return _trampoline0();`);
  await assert.rejects(run(()=>Promise.reject(new Error('host failed')),()=>{throw new Error('allocated on failure');},
    {reject(){rejected++;},exit(){exited++;},completionPromise(){return new Promise(()=>{});}}),/host failed/);
  assert.equal(rejected,1);assert.equal(exited,1);
});
