import {test} from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {QueryPool} from './succinct/pool.mjs';

test('query cancellation and errors join every reader before closing',async()=>{
  const workers=[];
  class FakeWorker extends EventEmitter {
    job=null;terminated=false;
    postMessage(m){
      if(m.type==='run')this.job=m;
      else queueMicrotask(()=>this.emit('message',{id:m.id,type:m.type==='init'?'ready':'closed'}));
    }
    terminate(){this.terminated=true;return Promise.resolve(0);}
  }
  const memory=new WebAssembly.Memory({initial:1,maximum:1,shared:true});
  const pool=await QueryPool.create(null,memory,0,2,{createWorker:async()=>{const w=new FakeWorker();workers.push(w);return w;}});
  const abort=new AbortController(),reason=new Error('cancelled');
  let settled=false,closed=false;
  const pending=pool.query(Int32Array.from([0,0,0,0,0,0,0,0]),{signal:abort.signal});
  const rejection=assert.rejects(pending,e=>e===reason).then(()=>{settled=true;});
  abort.abort(reason);
  const closing=pool.close().then(()=>{closed=true;});
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal(settled,false);assert.equal(closed,false);
  workers[0].emit('message',{id:workers[0].job.id,type:'error',message:'first reader failed'});
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal(settled,false);assert.equal(closed,false);
  workers[1].emit('message',{id:workers[1].job.id,type:'result',value:null});
  await rejection;await closing;
  assert.ok(workers.every(w=>w.terminated));
});

test('failed worker termination keeps the pool closed and permits cleanup retry',async()=>{
  let attempts=0;
  class BrokenTermination extends EventEmitter {
    postMessage(m){queueMicrotask(()=>this.emit('message',{id:m.id,type:m.type==='init'?'ready':'closed'}));}
    terminate(){return ++attempts===1?Promise.reject(new Error('termination failed')):Promise.resolve(0);}
  }
  const memory=new WebAssembly.Memory({initial:1,maximum:1,shared:true});
  const pool=await QueryPool.create(null,memory,0,1,{createWorker:async()=>new BrokenTermination()});
  await assert.rejects(pool.close(),/cleanup/);
  await assert.rejects(pool.query(new Int32Array()),/closed/);
  await pool.close();
  assert.equal(attempts,2);
});
