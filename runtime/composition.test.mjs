import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, readFile, writeFile, rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {build} from './build.mjs';

test('composed components use native values, separate instances and acknowledged buffers', {timeout:120000}, async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-composition-'));
  try {
    await build('examples/components/main.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    const memories=[];
    const {api}=await instantiate({instantiateCore:async(m,imports)=>{
      const x=await WebAssembly.instantiate(m,imports);
      if(x.exports.live)memories.push(x.exports);
      return x;
    }});
    assert.equal(api.add(20,22),42);
    assert.equal(api.identity(42n),42n);
    assert.equal(await api.run(),41);assert.equal(await api.run(),42);
    assert.equal(await api.otherCounter.next(),1);
    assert.equal(await api.run(),43);
    assert.equal(await api.consume(Promise.resolve(42)),42);
    for(let i=0;i<20;i++) {
      const bytes=Uint8Array.from({length:i*4096},(_,j)=>j);
      assert.deepEqual(api.echo(bytes),bytes);
      const text='日本😀'.repeat(i*512);
      assert.equal(await(await api.readyText(text)),text);
    }
    const stream=await api.prepend((async function*(){yield 8;yield 9;})());
    const items=[];for await(const batch of stream)items.push(...batch);
    assert.deepEqual(items,[7,8,9]);
    await new Promise(resolve=>setTimeout(resolve,25));
    assert.equal(memories.length,2);
    for(const memory of memories)assert.equal(memory.live(),0,'retained linear ABI buffer');
    const manifest=JSON.parse(await readFile(join(dir,'component.wasm.json'),'utf8'));
    assert.equal(manifest.composition,true);
    assert.deepEqual(manifest.imports,[]);
    assert.ok(!manifest.memoryGraph.unresolved.length,JSON.stringify(manifest.memoryGraph));
    assert.equal(await (await instantiate()).api.run(),41);
    // Native failure may poison an instance; disposal owns its remaining
    // buffers. Do not free them while a canonical copy could still be pending.
    const failed=await instantiate();
    await assert.rejects(failed.api.consume(Promise.reject(new Error('host failed'))));
    assert.equal(await (await instantiate()).api.consume(Promise.resolve(42)),42);
  } finally {await rm(dir,{recursive:true,force:true});}
});

test('async @connect forwards owned futures across component instances',{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-async-bind-'));
  try {
    await build('tests/component-fixtures/composition-async.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    const {api}=await instantiate();
    assert.equal(await api.run(Promise.resolve(42)),42);
  } finally {await rm(dir,{recursive:true,force:true});}
});

test('adapter memory and source data memory stay separate on both backends',{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-data-memory-'));
  const lib=`(@use "walz:core" (type $bytes)) (memory $data 1)
    (func $data (param $value i32) (result i32) (@effect reads writes)
      (block (result i32) (i32.store $data 0 $value) (i32.load $data 0)))
    (func $echo (param $value $bytes) (result $bytes) (@effect pure) $value)`;
  try {
    for(const backend of ['core','component']) {
      const source=join(dir,backend+'.walz'),out=join(dir,backend);
      await writeFile(source,backend==='core'?`(module ${lib} (export "data" (func $data)) (export "echo" (func $echo)))`:`(component (@lib ${lib}) (@expose "data" $data) (@expose "echo" $echo))`);
      const builder=backend==='core'?(await import('./build-core.mjs')).build:build;
      await builder(source,out);
      const {instantiate}=await import(pathToFileURL(join(out,'index.js')));
      const {api}=await instantiate();
      for(let n=0;n<10;n++) {
        assert.equal(api.data(n),n);
        const input=Uint8Array.from({length:65536},(_,i)=>i);
        assert.deepEqual(api.echo(input),input);
      }
    }
  } finally {await rm(dir,{recursive:true,force:true});}
});
