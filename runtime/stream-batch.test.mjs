import test from 'node:test';
import assert from 'node:assert/strict';
import {createChannels} from './channels.mjs';
import {mkdtemp,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
test('batch writes report accepted prefixes and cancellation releases the remainder',async()=>{
  const channels=createChannels(),writer=channels.stream(2),reader=channels.reader(writer);
  const write=channels.sendMany(writer,[1,2,3,4,5]);
  assert.deepEqual(await reader.readMany(2),{items:[1,2],done:false});
  await write[Symbol.asyncDispose]();
  assert.deepEqual(await write,[4,1]);
  channels.close(writer);
  assert.deepEqual(await reader.readMany(8),{items:[3,4],done:true});
  assert.deepEqual(await reader.readMany(8),{items:[],done:true});
  assert.equal(channels.active(),0);
});
test('rendezvous batches make progress without waiting for a full read',async()=>{
  const channels=createChannels(),writer=channels.stream(0),reader=channels.reader(writer);
  const read=reader.readMany(64);
  assert.deepEqual(await channels.sendMany(writer,[]),[0,0]);
  assert.equal(await channels.send(writer,42),0);
  assert.deepEqual(await read,{items:[42],done:false});
  const write=channels.sendMany(writer,[1,2,3]);
  assert.deepEqual(await reader.readMany(2),{items:[1,2],done:false});
  channels.close(writer);assert.deepEqual(await write,[2,1]);
  assert.deepEqual(await reader.next(),{done:true});
  assert.equal(channels.active(),0);
});
for(const backend of ['core','component'])test(`${backend}: explicit batch reads preserve tails and reduce host crossings`,{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-stream-batch-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/stream-batch.walz',dir);
    const {instantiate,TaskCancelledError}=await import(pathToFileURL(join(dir,'index.js')));
    let allocator,waiting;
    const {api,start}=await instantiate({onTaskEvent:e=>{if(e.type==='read-pending')waiting?.resolve();},instantiateCore:async(module,imports)=>{const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)allocator=instance.exports;return instance;}});
    assert.equal(await api.local(),42);
    for(const [method,items]of [['readText',['漢','😀','']],['readUnit',[undefined,undefined,undefined]]]){
      const [tail,batch]=await api[method]({[Symbol.asyncIterator](){return this;},async readMany(){return {items,done:true};},async return(){return {done:true};}},4);
      assert.equal(batch.length,3);if(method==='readText')assert.deepEqual(batch,items);await tail.return();
    }
    let reads=0,index=0,closed=0;
    const source={
      [Symbol.asyncIterator](){return this;},
      next(){throw new Error('batch transport must use readMany');},
      async readMany(max){reads++;assert.equal(max,13);const items=Array.from({length:Math.min(max,511-index)},()=>index++);return {items,done:index===511};},
      async return(){closed++;return {done:true};},
    };
    let stream=source;const values=[];
    for(;;){const[tail,batch]=await api.read(stream,13);stream=tail;if(!batch.length)break;values.push(...batch);}
    await stream.return();
    assert.deepEqual(values,Array.from({length:511},(_,i)=>i));assert.equal(reads,40);assert.equal(closed,1);assert.equal(allocator.live(),0);
    const channels=createChannels(),writer=channels.stream(0),reader=channels.reader(writer);
    const write=channels.sendMany(writer,[20,22,99]);
    const[tail,batch]=await api.read(reader,2);assert.deepEqual(Array.from(batch),[20,22]);
    await tail.return();assert.deepEqual(await write,[2,1]);channels.close(writer);assert.equal(channels.active(),0);
    const abandoned=channels.stream(0),pendingReader=channels.reader(abandoned);
    waiting=Promise.withResolvers();const task=start('read',[pendingReader,64]);await waiting.promise;
    await task.cancel('batch cancelled');await assert.rejects(task.result,TaskCancelledError);channels.close(abandoned);
    assert.equal(channels.active(),0);assert.equal(allocator.live(),0);
    assert.equal(await api.local(),42);
  }finally{await rm(dir,{recursive:true,force:true});}
});
