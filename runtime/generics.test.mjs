import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

for(const backend of ['core','component'])test(`${backend}: monomorphized list map and fold retain concrete export ABIs`,async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-generics-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('examples/generic-lists.wlt',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));let memory;
    const {api}=await instantiate({instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);
      if(instance.exports.live)memory=instance.exports;
      return instance;
    }});
    assert.equal(api.main(),42n);
    for(const xs of [[],[1n,2n,3n],Array.from({length:1024},(_,i)=>BigInt(i))]){
      const result=api.scale(xs,-7n);
      assert.deepEqual(result,new BigInt64Array(xs.map(x=>x*-7n)));
      assert.deepEqual(api.scalePure(xs,-7n),result);
      assert.equal(api.sum(result),xs.reduce((a,b)=>a+b,0n)*-7n);
      assert.deepEqual(api.identity(xs),new BigInt64Array(xs));
      assert.deepEqual(api.labels(xs,'日本語🦀'),xs.map(()=>'日本語🦀'));
      assert.equal(memory.live(),0);
    }
  }finally{await rm(dir,{recursive:true,force:true});}
});
