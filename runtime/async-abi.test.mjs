import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
for(const backend of ['core','component'])test(`${backend}: optimization preserves the async ABI without a dead host import`,async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-async-abi-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/async-abi.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    const {api}=await instantiate({eventLoop:{sleep:()=>assert.fail('unreachable host call')}});
    const value=api.main();assert.ok(value instanceof Promise);assert.equal(await value,42n);
  }finally{await rm(dir,{recursive:true,force:true});}
});
