import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {buildNative} from './build-native.mjs';
test('native async world creates future and stream using canonical producers',{timeout:60000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'wlt-native-async-'));
 try{
  await buildNative('tests/component-fixtures/native-async.wlt','tests/component-fixtures/native-async.wit',dir,{});
  const {instantiate}=await import(pathToFileURL(join(dir,'native.js')));
  const memories=[];
  const component=await instantiate(async name=>WebAssembly.compile(await readFile(join(dir,name))),{},async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;});
  const api=component['test:native/api'];
  assert.equal(await api.consume(Promise.resolve(42)),42);
  const future=await api.ready();assert.equal(await future,42);
  const stream=await api.prepend((async function*(){yield 8;yield 9;})());
  const items=[];for await(const batch of stream)items.push(...batch);assert.deepEqual(items,[7,8,9]);
  for(let n=0;n<10;n++){const value='日本😀'.repeat(n===0?20000:1024);assert.equal(await(await api.readyText(value)),value);}
  await new Promise(r=>setTimeout(r,25));assert.equal(memories.length,2);for(const memory of memories)assert.equal(memory.live(),0,'native producer retained an ABI allocation');
 }finally{await rm(dir,{recursive:true,force:true});}
});
