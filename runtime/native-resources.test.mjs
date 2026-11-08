import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {buildNative} from './build-native.mjs';
import {collect} from './testing/memory.mjs';
test('native resource own/borrow preserves identity and calls the host destructor once',{timeout:60000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'wlt-native-resource-'));
 try{
  await buildNative('tests/component-fixtures/native-resources.wlt','tests/component-fixtures/native-resources.wit',dir,{bindings:'tests/component-fixtures/native-resources.json'});
  const {instantiate}=await import(pathToFileURL(join(dir,'native.js')));
  let closed=0,memory;
  class File {constructor(){this.value=42;this.closed=false;}[Symbol.dispose](){assert.equal(this.closed,false);this.closed=true;closed++;}}
  const host={File,make:()=>new File(),read:file=>{assert.equal(file.closed,false);return file.value;}};
  const component=await instantiate(async name=>WebAssembly.compile(await readFile(join(dir,name))),{'test:resources/host':host},async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memory=x.exports;return x;});
  const api=component['test:resources/api'];
  for(let n=0;n<20;n++){
    const file=api.make();assert.ok(file instanceof File);assert.equal(api.read(file),42);assert.equal(closed,n,'borrow must not close its owner');
    const result=api.echo(file);assert.equal(result,file);api.close(result);assert.equal(closed,n+1);assert.equal(memory.live(),0);
  }
  const optional=api.make();assert.equal(api.optional(optional),optional);assert.equal(closed,20);api.releaseOption(optional);api.releaseOption(undefined);assert.equal(closed,21);
  if(global.gc){const ref=(()=>{const file=api.make();const ref=new WeakRef(file);api.close(file);return ref;})();await collect();assert.equal(ref.deref(),undefined,'closed canonical resource retained its host object');}
 }finally{await rm(dir,{recursive:true,force:true});}
});
