import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,readFile,writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {build} from './build.mjs';
import {buildWasmUse} from './testing/wasm-use.mjs';

test('explicit binary instances share aliases, isolate state, and copy Canonical ABI values',async()=>{
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
    assert.equal(await api.main(),3);assert.equal(await api.main(),7);
    assert.equal(await api.otherNext(),1,'a second instantiate creates independent state');
    assert.equal(await api.main(),11,'the facade and direct caller share an explicit provider');
    assert.equal(await api.double(21n),42n);
    assert.equal(await api.consume(Promise.resolve(42n)),42n);
    assert.equal(await(await api.later(42n)),42n);
    for(const value of ['', '日本語😀', 'x'.repeat(100000)])assert.equal(await api.text(value),value);
    const bytes=Uint8Array.of(0,1,128,255);assert.deepEqual(await api.data(bytes),bytes);
    assert.deepEqual(await api.point({x:-7,y:42}),{x:-7,y:42});
    assert.equal(await api.unsigned(0xffffffff),0xffffffff);
    assert.equal(await api.optional(undefined),undefined);assert.equal(await api.optional(42),42);
    assert.equal(await api.result({tag:'ok',val:42n}),42n);
    await assert.rejects(api.result({tag:'err',val:'失敗'}),error=>error.payload==='失敗');
    assert.equal(await api.localFuture(),42n,'local Future results must remain values');
    await new Promise(resolve=>setTimeout(resolve,25));
    assert.ok(memories.length>=2,'observe both the consumer and foreign ABI arenas');
    for(const memory of memories)assert.equal(memory.live(),0,'binary boundary retained an ABI allocation');
    const manifest=JSON.parse(await readFile(join(directory,'component.wasm.json'),'utf8'));
    assert.equal(manifest.imports.length,0);
    assert.equal(manifest.composition,true);
  }finally{await rm(directory,{recursive:true,force:true});}
});

test('constructor selection has no start effects; unused explicit instantiation still traps',async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-binary-start-'));
  try {
    const {execFile}=await import('node:child_process');
    const {promisify}=await import('node:util');
    const source=join(directory,'trap.walz');
    await writeFile(source,'(component (core module $M (func $start unreachable) (start $start)) (core instance (instantiate $M)))');
    await promisify(execFile)('cargo',['run','--locked','--quiet','--','build',source,'-o',join(directory,'trap.wasm')]);
    for(const [name,extra] of [['selected',''],['instantiated','(instance $unused (instantiate $Trap))']]) {
      const entry=join(directory,name+'.walz');
      await writeFile(entry,`(component (@use "./trap.wasm" (component $Trap)) ${extra}
        (@lib (func $answer (result i32) (@effect pure) 42)) (@expose "answer" $answer))`);
      const output=join(directory,name);await build(entry,output);
      const {instantiate}=await import(pathToFileURL(join(output,'index.js')));
      if(extra)await assert.rejects(instantiate(),/unreachable/);
      else assert.equal((await instantiate()).api.answer(),42);
    }
  }finally{await rm(directory,{recursive:true,force:true});}
});
