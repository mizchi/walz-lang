import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';

for (const backend of ['core','component']) test(`${backend}: scalar newtypes erase through collections and delivery ABIs`,async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-language-core-'));
  try {
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/language-core.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    const {api}=await instantiate();
    for(const value of [0n,-1n,9223372036854775807n]) assert.equal(api.id(value),value);
    assert.equal(api.small(-2147483648),-2147483648);
    assert.equal(api.float(1.5),1.5);assert.equal(api.weight(-0),-0);
    assert.deepEqual([...api.ids([1n,42n,-1n])],[1n,42n,-1n]);
    assert.deepEqual([...api.ints([3n,4n])],[3n,4n]);
    assert.equal(await api.readyId(42n),42n);assert.equal(await api.readyInt(7n),7n);
    assert.equal(api.maybeId(42n),42n);assert.equal(api.maybeInt(undefined),undefined);
    assert.deepEqual(api.compute(0),{tag:'ok',val:42n});
    assert.deepEqual(api.compute(1),{tag:'err',val:7});
    assert.equal(api.sqrt(1764),42);assert.equal(api.sqrt(-1),0);
  } finally {await rm(dir,{recursive:true,force:true});}
});
