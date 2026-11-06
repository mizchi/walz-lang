import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

for(const backend of ['core','component'])test(`${backend}: named structs preserve field order, text, exceptions and suspensions`,async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-struct-'));
  try {
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/struct-construction.wlt',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    let arena;
    const {api}=await instantiate({instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);
      if(instance.exports.live)arena=instance.exports;
      return instance;
    }});
    for(let i=0;i<32;i++) {
      const name=`Alice 日本語🦀 ${i}`,email=`alice${i}@example.com`;
      assert.deepEqual(api.profile(name,30+i,email),[name,[30+i,email]]);
      assert.deepEqual(api.literalProfile(),['日本語🦀',[30,'alice@example.com']]);
      assert.equal(api.boxed(),42n);
      assert.equal(api.shorthand(41n),42n);
      assert.equal(api.order(),2112);
      assert.equal(api.throwBeforeLater(0),231);
      assert.equal(api.throwBeforeLater(1),2);
      assert.equal(api.earlyReturn(0),42);assert.equal(api.earlyReturn(1),42);
      assert.equal(api.branch(),42);
      assert.equal(await api.awaitSum(Promise.resolve(2)),42);
      assert.equal(arena.live(),0);
    }
  } finally {await rm(dir,{recursive:true,force:true});}
});
