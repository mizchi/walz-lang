import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,writeFile,readFile} from 'node:fs/promises';
import {openSync,readFileSync,closeSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {Resources} from './resources.mjs';

test('resource registry rejects wrong kinds, stale IDs, and foreign task scopes',()=>{
  let task={}; const closed=[];
  const resources=new Resources(()=>task,()=>{});
  const id=resources.add('file',42,v=>closed.push(v));
  assert.throws(()=>resources.use('socket',id,()=>{},[]),/resource/);
  const original=task; task={};
  assert.throws(()=>resources.close('file',id),/resource/);
  task=original; resources.close('file',id);
  assert.throws(()=>resources.close('file',id),/resource/);
  assert.deepEqual(closed,[42]); assert.equal(resources.entries.size,0);
});

for(const backend of ['core','component'])test(`${backend}: resources close exactly once on every exit`,{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-resource-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('examples/resources/main.wlt',dir,{bindings:'examples/resources/policy.json'});
    const {instantiate,TaskCancelledError}=await import(pathToFileURL(join(dir,'index.js')));
    let next=0,closed=[],pending;
    const {api,start}=await instantiate({imports:{files:{
      open:()=>({id:++next}),read:f=>f.id,close:f=>closed.push(f.id),
    }},eventLoop:{sleep:()=>{pending=Promise.withResolvers();pending.promise[Symbol.asyncDispose]=()=>pending.resolve();return pending.promise;}}});
    assert.equal(api.normal(),1);assert.deepEqual(closed,[1]);
    assert.deepEqual(api.err(),{tag:'err',val:7});assert.deepEqual(closed,[1,2]);
    api.explicit();api.nested();assert.deepEqual(closed,[1,2,3,5,4]);
    assert.equal(api.caught(),42);assert.deepEqual(closed,[1,2,3,5,4,6]);
    const task=start('pending');
    for(let i=0;i<50&&!pending;i++)await Promise.resolve();
    await task.cancel('stop');await assert.rejects(task.result,TaskCancelledError);
    assert.deepEqual(closed,[1,2,3,5,4,6,7]);
    await task.cancel();assert.equal(closed.length,7);
    const trapped=start('trap');await assert.rejects(trapped.result);
    assert.equal(closed.at(-1),8);
    const queued=start('normal');await queued.cancel();await assert.rejects(queued.result,TaskCancelledError);
    assert.equal(next,8);
    // A real descriptor uses the same synchronous destructor contract.
    const file=join(dir,'data');await writeFile(file,'42');let opened=0,disposed=0;
    const fs=await instantiate({imports:{files:{open:()=>{opened++;return openSync(file,'r');},read:fd=>Number(readFileSync(fd,'utf8')),close:fd=>{disposed++;closeSync(fd);}}}});
    assert.equal(fs.api.normal(),42);assert.equal(opened,disposed);
    // Closing one resource may fail; cleanup still attempts all remaining ones.
    let attempts=0;
    const broken=await instantiate({imports:{files:{open:()=>({}),read:()=>0,close:()=>{attempts++;throw new Error('close failed');}}}});
    const failure=broken.start('nested');await assert.rejects(failure.result,/close failed|cleanup/);
    assert.equal(attempts,2);
    assert.throws(()=>broken.api.normal());assert.equal(attempts,2);
    assert.ok((await readFile(join(dir,backend==='core'?'manifest.json':'component.wasm.json'),'utf8')).includes('resource'));
  }finally{await rm(dir,{recursive:true,force:true});}
});
