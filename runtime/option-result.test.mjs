import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instrument} from './testing/memory.mjs';
for(const backend of ['core','component'])test(`${backend}: string sugar and core combinators preserve evaluation and identity`,{timeout:60000},async()=>{
 const directory=await mkdtemp(join(tmpdir(),'wlt-option-result-'));
 try{
  const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
  await build('tests/component-fixtures/option-result.wlt',directory);await instrument(directory,backend);
  const {instantiate}=await import(pathToFileURL(join(directory,'index.js')));let memory;
  const {api,__memory}=await instantiate({instantiateCore:async(m,i)=>{const instance=await WebAssembly.instantiate(m,i);if(instance.exports.live)memory=instance.exports;return instance;}});
  for(let i=0;i<10;i++){
   assert.deepEqual(api.supply('日本😀'),[{tag:'ok',val:'日本😀'},1]);
   assert.deepEqual(api.supply(undefined),[{tag:'err',val:7},1]);
   assert.deepEqual(api.mapped({tag:'ok',val:42n},5),[{tag:'ok',val:42n},0]);
   assert.deepEqual(api.mapped({tag:'err',val:7},5),[{tag:'err',val:12},1]);
   assert.deepEqual([...api.literal()],['日本😀','','quote"']);assert.equal(api.identity(),1);
   assert.equal(memory.live(),0);assert.ok(Object.values(__memory()).every(n=>n===0));
  }
 }finally{await rm(directory,{recursive:true,force:true});}
});
