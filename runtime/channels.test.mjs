import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createChannels} from './channels.mjs';
import {mkdtemp,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';

test('bounded stream backpressure, draining close, and one reader', async()=>{
  const channels=createChannels(), writer=channels.stream(1), reader=channels.reader(writer);
  assert.throws(()=>channels.reader(writer),/reader/);
  assert.equal(await channels.send(writer,11),0);
  let delivered=false;
  const pending=channels.send(writer,22).then(code=>{delivered=true;return code;});
  await Promise.resolve();assert.equal(delivered,false);
  assert.throws(()=>channels.send(writer,33),/concurrent/);
  assert.deepEqual(await reader.next(),{done:false,value:11});
  assert.equal(await pending,0);
  channels.close(writer);channels.close(writer);
  assert.deepEqual(await reader.next(),{done:false,value:22});
  assert.deepEqual(await reader.next(),{done:true});
  assert.equal(await channels.send(writer,33),1);
  assert.equal(channels.active(),0);
});

test('rendezvous reader cancellation and write disposal release pending values',async()=>{
  const channels=createChannels(), writer=channels.stream(0), reader=channels.reader(writer);
  const cancelled=channels.send(writer,{large:'payload'});
  await cancelled[Symbol.asyncDispose]();
  assert.equal(await cancelled,1);
  const write=channels.send(writer,42);
  assert.deepEqual(await reader.next(),{done:false,value:42});assert.equal(await write,0);
  const abandoned=channels.send(writer,99);
  await reader.return();assert.equal(await abandoned,1);
  channels.close(writer);assert.equal(channels.active(),0);
  const other=channels.stream(0), end=channels.reader(other), read=end.next();
  assert.throws(()=>end.next(),/concurrent/);
  channels.close(other);assert.deepEqual(await read,{done:true});
  assert.equal(channels.active(),0);
});

test('future resolves once, closes unresolved, and reader disposal closes the channel',async()=>{
  const channels=createChannels(), writer=channels.future(), future=channels.reader(writer);
  assert.equal(channels.resolve(writer,42),0);
  assert.equal(channels.resolve(writer,43),1);
  assert.deepEqual(await future,{tag:'ok',val:42});
  channels.close(writer);assert.equal(channels.active(),0);
  const abandoned=channels.future(), interrupted=channels.reader(abandoned);
  channels.close(abandoned);assert.deepEqual(await interrupted,{tag:'err',val:1});
  const cancelled=channels.future(), reading=channels.reader(cancelled);
  await reading[Symbol.asyncDispose]();
  assert.equal(channels.resolve(cancelled,10),1);
  assert.deepEqual(await reading,{tag:'err',val:1});
  channels.close(cancelled);assert.equal(channels.active(),0);
});

test('invalid capacities and foreign handles fail closed',()=>{
  const channels=createChannels(), other=createChannels();
  for(const n of [-1,1.5,65537,Infinity])assert.throws(()=>channels.stream(n),/capacity/);
  assert.throws(()=>channels.reader(other.stream(0)),/channel/);
  assert.throws(()=>channels.close({}),/channel/);
  channels.closeAll();other.closeAll();assert.equal(other.active(),0);
});

for(const backend of ['core','component'])test(`${backend}: affine writers, backpressure and cancellation free resources`,{timeout:60000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-channels-'));
  try {
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('examples/channels/main.walz',directory,{bindings:'examples/channels/policy.json'});
    const {instantiate,TaskCancelledError}=await import(pathToFileURL(join(directory,'index.js')));
    const channels=createChannels(), values=[];
    let completed, allocator, pending;
    const {api,start}=await instantiate({imports:{channels:{...channels,
      streamReader:channels.reader,futureReader:channels.reader,
      closeStream:channels.close,closeFuture:channels.close,
    },consumer:{start:reader=>{completed=(async()=>{for await(const n of reader)values.push(n);})();}}},
      onTaskEvent:event=>{if(event.type==='host-pending')pending?.resolve();},
      instantiateCore:async(module,imports)=>{const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)allocator=instance.exports;return instance;},
    });
    assert.equal(await api.produce(),0);await completed;assert.deepEqual(values,[20,22]);
    assert.equal(await api.pipeline(),42,'guest producer and consumer run concurrently until EOF');
    assert.deepEqual(await api.resolved(),{tag:'ok',val:42});
    assert.equal(channels.active(),0);assert.equal(allocator.live(),0);
    for(let i=0;i<20;i++) {
      pending=Promise.withResolvers();const task=start('pending');
      // The host wrapper publishes a pending event after registering disposal.
      await pending.promise;
      await task.cancel('stop');await assert.rejects(task.result,TaskCancelledError);
      assert.equal(channels.active(),0);assert.equal(allocator.live(),0);
    }
    assert.deepEqual(await api.resolved(),{tag:'ok',val:42});
  }finally{await rm(directory,{recursive:true,force:true});}
});
