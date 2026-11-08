import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {build} from './build-core.mjs';

test('readonly Core contract requires opt-in and checks logical bounds before loading once',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-readonly-'));
  try{
    const source=`(module (%race-free)
      (import "env" "memory" (memory $m 1 1 shared))
      (%atomic.cell $calls $m i32 0)
      (%readonly.region $data $m 16 64)
      (func $word (export "word") (param $offset i32) (result i32) (%effect reads writes synchronizes)
        (%readonly.load $data i32 (block (result i32)
          (drop (i32.atomic.rmw.add (%atomic.at $calls) 1)) $offset)))
      (func $byte (export "byte") (param $offset i32) (result i32) (%effect reads)
        (%readonly.load $data u8 $offset)))`;
    const file=join(dir,'source.wlt');await writeFile(file,source);await build(file,join(dir,'out'));
    const {instantiate}=await import(pathToFileURL(join(dir,'out/index.js')));
    const memory=new WebAssembly.Memory({initial:1,maximum:1,shared:true});
    const bytes=new Uint8Array(memory.buffer);bytes[16]=42;bytes[79]=255;
    const options={imports:{env:{memory}}};
    await assert.rejects(instantiate(options),/readonlyHostContract/);
    await assert.rejects(instantiate({...options,readonlyHostContract:'wlt-atomic-cells-v1'}),/readonlyHostContract/);
    const good={...options,readonlyHostContract:'wlt-readonly-regions-v1'};
    const {api}=await instantiate(good);
    assert.equal(api.word(0),42);assert.equal(Atomics.load(new Int32Array(memory.buffer),0),1);
    assert.equal(api.byte(63),255);
    for(const offset of [61,64,-1,0x7fffffff]){
      const {api}=await instantiate(good);
      assert.throws(()=>api.word(offset),WebAssembly.RuntimeError);
    }
    assert.equal(Atomics.load(new Int32Array(memory.buffer),0),5);
  }finally{await rm(dir,{recursive:true,force:true});}
});
