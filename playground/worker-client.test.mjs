import test from 'node:test';
import assert from 'node:assert/strict';
import {WorkerClient} from './worker-client.mjs';

for(const kind of ['onerror','onmessageerror'])test(`a replaced worker's ${kind} cannot cancel the new compiler`,async()=>{
  const original=globalThis.Worker,workers=[];
  globalThis.Worker=class{
    constructor(){workers.push(this);}
    postMessage(message){this.message=message;}
    terminate(){this.terminated=true;}
  };
  const client=new WorkerClient('compiler-worker.mjs',{timeout:10000});
  try{
    const first=client.run({engine:'rust'});client.cancel();await assert.rejects(first,/Stopped/);
    const second=client.run({engine:'selfhost'}).then(value=>({value}),error=>({error}));
    workers[0][kind]({message:'late failure',preventDefault(){}});
    workers[1].onmessage({data:{id:workers[1].message.id,result:42}});
    assert.deepEqual(await second,{value:42});
    assert.equal(workers[1].terminated,undefined);
  }finally{client.cancel();globalThis.Worker=original;}
});
