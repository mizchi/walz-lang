import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

for(const backend of ['core','component'])test(`${backend}: scalar type classes specialize across portable and async ABIs`,async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-classes-'));
  try {
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/typeclasses.wlt',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    let arena;
    const {api}=await instantiate({instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);
      if(instance.exports.live)arena=instance.exports;
      return instance;
    }});
    for(let i=0;i<32;i++) {
      assert.equal(api.add32(0x7fffffff,1),-2147483648);
      assert.equal(api.add64(0x7fffffffffffffffn,1n),-0x8000000000000000n);
      assert.equal(api.float32(16777216,1),16777216);
      assert.equal(api.float64(0.1,0.2),0.1+0.2);
      assert.equal(api.arithmetic32(20,6,3,1),42);
      assert.equal(api.arithmetic64(20n,6n,3n,1n),42n);
      assert.equal(api.arithmeticF32(20,6,3,1),42);
      assert.equal(api.arithmeticF64(20,6,3,1),42);
      assert.equal(api.arithmetic32(7,0,1,-2),-3);
      assert.equal(api.arithmetic64(7n,0n,1n,-2n),-3n);
      assert.equal(api.arithmeticF64(7,0,1,-2),-3.5);
      assert.equal(api.arithmeticF32(1,0,1,0),Infinity);
      assert.ok(Number.isNaN(api.arithmeticF64(0,0,1,0)));
      assert.equal(api.less32(-1,0),1);assert.equal(api.less64(-1n,0n),1);
      assert.equal(api.eq64(1n<<40n,0n),0);assert.equal(api.eq64(-1n,-1n),1);
      assert.equal(api.ne32(1,2),1);
      assert.equal(api.literals(),4);assert.equal(api.inferred(40n),42n);
      assert.deepEqual(api.pair(-1n),[0n,1]);
      assert.equal(await api.awaitAdd(Promise.resolve(40n)),42n);
      assert.equal(api.both32(2,4),1);
      assert.equal(api.bits32(0xf0,0x3c),0xcc);
      assert.equal(api.bits64(1n<<60n,3n),(1n<<60n)^3n);
      assert.equal(api.shl32(1,32),1);assert.equal(api.shl64(1n,64n),1n);
      assert.equal(api.shl32(1,-1),-2147483648);assert.equal(api.shl64(1n,-1n),-0x8000000000000000n);
      assert.equal(api.ashift32(-1,1),-1);assert.equal(api.ashift64(-1n,1n),-1n);
      assert.equal(api.ushift32(-1,1),2147483647);assert.equal(api.ushift64(-1n,1n),0x7fffffffffffffffn);
      assert.deepEqual(api.logicalTrace(0,7,0),[0,1]);
      assert.deepEqual(api.logicalTrace(2,7,0),[1,12]);
      assert.deepEqual(api.logicalTrace(-1,7,1),[1,1]);
      assert.deepEqual(api.logicalTrace(0,0,1),[0,12]);
      assert.equal(api.shortTrap(7),1);
      assert.deepEqual(await api.asyncShort(1),[1,0]);
      assert.deepEqual(await api.asyncShort(0),[1,1]);
      assert.equal(arena.live(),0);
    }
    // Reaching a trap poisons the Component instance. Keep the successful
    // short-circuit/async checks on their own instance.
    const {api:trapping}=await instantiate();
    assert.throws(()=>trapping.shortTrap(0));
    const {api:dividing}=await instantiate();
    assert.throws(()=>dividing.arithmetic32(1,0,1,0));
    await assert.rejects(api.awaitAdd(Promise.reject(new Error('producer failed'))),/promise failed/);
    assert.equal(arena.live(),0);
  } finally {await rm(dir,{recursive:true,force:true});}
});
