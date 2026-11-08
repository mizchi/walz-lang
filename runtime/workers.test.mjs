import {test} from 'node:test';
import assert from 'node:assert/strict';
import {WorkerHost} from './workers/host.mjs';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {collect} from './testing/memory.mjs';
test('general Worker host schedules unrelated asynchronous jobs and transfers results',async()=>{
  const pool=await WorkerHost.create({workers:2,driver:new URL('./testing/worker-driver.mjs',import.meta.url),init:{bias:2}});
  try{
    const a=pool.run({numbers:[1,2,3],delay:3});
    const b=pool.run({numbers:[10],delay:0});
    assert.deepEqual(await Promise.all([a,b]),[8,12]);
    const out=await pool.map(Array.from({length:7},(_,i)=>({bytes:Uint8Array.of(i,i+1)})));
    assert.deepEqual(out.map(a=>[...a]),Array.from({length:7},(_,i)=>[i+2,i+3]));
    const abort=new AbortController(),reason=new Error('cancel one job');
    const pending=pool.run({numbers:[1],delay:1000},{signal:abort.signal});
    const unaffected=pool.run({numbers:[40],delay:2});
    abort.abort(reason);
    await assert.rejects(pending,e=>e===reason);
    assert.equal(await unaffected,42);
    await pool.join();
    for(const reason of [0,false,'',null]){
      const abort=new AbortController();
      const pending=pool.run({numbers:[1],delay:1000},{signal:abort.signal});
      abort.abort(reason);
      await assert.rejects(pending,e=>e===reason);
    }
    const task=pool.start({numbers:[1],delay:1000});
    const rejected=assert.rejects(task.result,e=>e==='stop');
    await task.cancel('stop');await rejected;
    const future=pool.start({numbers:[40]}).result;
    assert.equal(await future,42);await future[Symbol.asyncDispose]();
  }finally{await pool.close();}
});

test('CPU-bound jobs poll separate cancellation bits without stopping their sibling',async()=>{
  const memory=new WebAssembly.Memory({initial:1,maximum:1,shared:true}),words=new Int32Array(memory.buffer);
  const host=await WorkerHost.create({workers:2,driver:new URL('./testing/worker-driver.mjs',import.meta.url),
    init:{bias:0,control:memory},cancellation:{memory,byteOffset:0}});
  const abort=new AbortController(),reason=new Error('stop first CPU job');
  try{
    const first=host.run({cpu:true},{signal:abort.signal}),second=host.run({cpu:true});
    for(const index of [1,2]){
      const deadline=performance.now()+3000;
      while(!Atomics.load(words,index)){
        if(performance.now()>deadline)throw new Error('worker did not start');
        await Atomics.waitAsync(words,index,0,100).value;
      }
    }
    abort.abort(reason);await assert.rejects(first,e=>e===reason);
    assert.equal(Atomics.load(words,0)&2,0);
    Atomics.store(words,5,1);
    assert.deepEqual(await second,{cancelled:false,workerId:1});
  }finally{abort.abort(reason);Atomics.store(words,4,1);Atomics.store(words,5,1);await host.close();}
});

test('retaining completed task handles does not retain a closed Worker host',
  {skip:typeof global.gc!=='function'},async()=>{
    const completed=await (async()=>{
      const host=await WorkerHost.create({driver:new URL('./testing/worker-driver.mjs',import.meta.url),init:{bias:2}});
      const task=host.start({numbers:[40]});
      assert.equal(await task.result,42);
      await host.close();
      return{ref:new WeakRef(host),task};
    })();
    await collect();
    assert.equal(completed.ref.deref(),undefined);
    assert.equal(await completed.task.result,42);
    await completed.task.cancel();
  });

for(const backend of ['core','component'])test(`${backend}: general Workers run WLT JSPI futures and join their cancellation`,async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-worker-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/worker-job.wlt',dir);
    const pool=await WorkerHost.create({workers:2,driver:new URL('./testing/worker-wlt-driver.mjs',import.meta.url),init:{url:pathToFileURL(join(dir,'index.js')).href}});
    try{
      assert.deepEqual(await pool.map([{value:20n},{value:21n}]),[40n,42n]);
      const abort=new AbortController(),reason=new Error('cancel only this WLT job');
      const pending=pool.run({value:1n,ms:10000},{signal:abort.signal});
      const sibling=pool.run({value:21n,ms:1});abort.abort(reason);
      await assert.rejects(pending,e=>e===reason);assert.equal(await sibling,42n);
    }finally{await pool.close();}
  }finally{await rm(dir,{recursive:true,force:true});}
});
