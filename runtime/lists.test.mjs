import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {instrument,tick} from './testing/memory.mjs';

for(const backend of ['core','component'])test(`${backend}: generic lists preserve values, snapshots, and ABI ownership`,{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-lists-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/lists.wlt',dir);await instrument(dir,backend);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    let memory;
    const make=(options={})=>instantiate({...options,instantiateCore:async(module,imports)=>{const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)memory=instance.exports;return instance;}});
    const {api}=await make();
    assert.deepEqual(api.literal(),new BigInt64Array([40n,2n]));assert.deepEqual(api.empty(),[]);
    assert.deepEqual(api.snapshot(),new BigInt64Array([40n,2n]));
    for(const input of [[],[0n,-1n,-9223372036854775808n,9223372036854775807n],Array.from({length:4096},(_,i)=>BigInt(i))]){
      assert.deepEqual(api.identity(input),new BigInt64Array(input));assert.equal(api.length(input),input.length);
      assert.deepEqual(api.slice(input,input.length,0),new BigInt64Array());
      assert.deepEqual(api.slice(input,0,input.length),new BigInt64Array(input));
      if(input.length)assert.equal(api.get(input,input.length-1),input.at(-1));
      assert.equal(memory.live(),0);
    }
    const texts=['','日本語🦀','\0','x'.repeat(65536)];
    assert.deepEqual(api.texts(texts),texts);
    const nested=[undefined,{tag:'ok',val:[-1n,'日本語']},{tag:'err',val:new Uint8Array([0,255])}];
    assert.deepEqual(api.nested(nested),nested);
    assert.deepEqual(api.units(['value','value']),['value','value']);
    assert.deepEqual(api.newUnits(),['value','value']);assert.equal(api.unitGet(['value'],0),undefined);
    const floats=new Float32Array([0,-0,Math.PI,NaN,Infinity]);
    assert.deepEqual(api.floats(floats),floats);
    const doubles=new Float64Array([0,-0,Math.PI,NaN,-Infinity]);assert.deepEqual(api.doubles(doubles),doubles);
    assert.deepEqual(api.matrix([[1,2],[],[3]]),[new Int32Array([1,2]),new Int32Array(),new Int32Array([3])]);
    assert.deepEqual(await api.await(Promise.resolve(texts)),texts);
    assert.deepEqual(await api.first([texts]),texts);assert.equal(await api.first([]),undefined);
    assert.equal(memory.live(),0);
    // Entry conversion failures must roll back partially encoded nested buffers.
    for(const input of (backend==='core'?[null,{},'text',[1n,2],Array(2)]:[null,[1n,2],Array(2)])){
      assert.throws(()=>api.identity(input),undefined,`invalid: ${String(input)}`);assert.equal(memory.live(),0);
    }
    assert.throws(()=>api.texts(['allocated','bad',1]));assert.equal(memory.live(),0);
    assert.deepEqual(api.identity([42n]),new BigInt64Array([42n]));
    const failing=await make();
    await assert.rejects(failing.api.await(Promise.resolve(['allocated',1])));await tick();assert.equal(memory.live(),0);assert.ok(Object.values(failing.__memory()).every(n=>n===0),JSON.stringify(failing.__memory()));
    let closed=0;
    const streamFailure=await make();
    const badStream={ [Symbol.asyncIterator](){return this;},async next(){return {done:false,value:['allocated',1]};},async return(){closed++;return {done:true};} };
    await assert.rejects(streamFailure.api.first(badStream),error=>{assert.doesNotMatch(error.message,/deadlock/);return true;});await tick();assert.equal(memory.live(),0);
    assert.ok(Object.values(streamFailure.__memory()).every(n=>n===0),JSON.stringify(streamFailure.__memory()));assert.equal(closed,1);
    let trapClosed=0;
    const trapScope=await make();
    const held={ [Symbol.asyncIterator](){return this;},async next(){return {done:false,value:['held']};},async return(){trapClosed++;return {done:true};} };
    await assert.rejects(trapScope.start('trapOwned',[held]).result);await tick();assert.equal(trapClosed,1);
    assert.equal(memory.live(),0);assert.ok(Object.values(trapScope.__memory()).every(n=>n===0));
    const reading=Promise.withResolvers(),control=Promise.withResolvers();
    control.promise[Symbol.asyncDispose]=()=>control.resolve(['late','日本語']);
    const cancellation=await make({onTaskEvent:e=>{if(e.type==='read-pending')reading.resolve();}});
    const pending=cancellation.start('await',[control.promise]);await reading.promise;
    await pending.cancel('stop');await assert.rejects(pending.result,{name:'TaskCancelledError'});
    await tick();assert.equal(memory.live(),0);assert.ok(Object.values(cancellation.__memory()).every(n=>n===0));
    for(const [method,args] of [['get',[[],0]],['unitGet',[[],0]],['get',[[1n],-1]],['slice',[[1n],2,0]],['slice',[[1n],0,-1]]]){
      const fresh=await make();assert.throws(()=>fresh.api[method](...args));assert.equal(memory.live(),0);
    }
  }finally{await rm(dir,{recursive:true,force:true});}
});
