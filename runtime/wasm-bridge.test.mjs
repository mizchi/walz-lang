import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
for(const backend of ['core','component'])test(`${backend}: validated memory and SIMD operations use the named guest memory`,async()=>{
  const directory=await mkdtemp(join(tmpdir(),'wlt-wat-'));
  try {
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/wasm-bridge.wlt',directory);
    const {instantiate}=await import(pathToFileURL(join(directory,'index.js')));
    let allocator;
    const {api}=await instantiate({instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)allocator=instance.exports;return instance;
    }});
    for(let i=0;i<1000;i++){assert.equal(api.main(),42);assert.equal(api.simd(),30);}
    assert.equal(allocator.live(),0);
    assert.throws(()=>api.trap(65536),/bounds|memory/);
    assert.equal(allocator.live(),0);
  }finally{await rm(directory,{recursive:true,force:true});}
});
