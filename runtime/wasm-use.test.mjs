import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {buildWasmUse} from './testing/wasm-use.mjs';

test('WIT-typed binary uses link one stateful instance and copy Canonical ABI values',async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-wasm-use-'));
  try{
    await buildWasmUse(directory);
    await rm(join(directory,'source'),{recursive:true,force:true});
    const memories=[];
    const {instantiate}=await import(pathToFileURL(join(directory,'index.js')));
    const runtime=await instantiate({instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);
      if(instance.exports.live)memories.push(instance.exports);
      return instance;
    }});
    const {api}=runtime;
    const first=runtime.start('main'),second=runtime.start('main');
    await Promise.resolve();
    assert.equal(second.state,'queued','manual JSPI must keep the first task active');
    assert.equal(await first.result,3);assert.equal(await second.result,7);
    const cancelled=runtime.start('main');
    const rejection=assert.rejects(cancelled.result,error=>error.name==='TaskCancelledError');
    await cancelled.cancel();await rejection;
    assert.equal(await api.main(),11,'a queued cancellation must not call the binary');
    for(const value of ['', '日本語😀', 'x'.repeat(100000)])assert.equal(await api.text(value),value);
    const bytes=Uint8Array.of(0,1,128,255);assert.deepEqual(await api.data(bytes),bytes);
    assert.deepEqual(await api.point({x:-7,y:42}),{x:-7,y:42});
    assert.equal(await api.unsigned(0xffffffff),0xffffffff);
    assert.equal(await api.optional(undefined),undefined);assert.equal(await api.optional(42),42);
    assert.deepEqual(await api.result({tag:'ok',val:42n}),{tag:'ok',val:42n});
    assert.deepEqual(await api.result({tag:'err',val:'失敗'}),{tag:'err',val:'失敗'});
    assert.equal(await api.localFuture(),42n,'local Future results must remain values');
    assert.ok(memories.length>=2,'observe both the consumer and foreign ABI arenas');
    for(const memory of memories)assert.equal(memory.live(),0,'binary boundary retained an ABI allocation');
    const manifest=JSON.parse(await readFile(join(directory,'component.wasm.json'),'utf8'));
    assert.equal(manifest.imports.length,0);
  }finally{await rm(directory,{recursive:true,force:true});}
});
