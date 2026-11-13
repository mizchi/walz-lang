import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {build} from './build.mjs';
import {collect} from './testing/memory.mjs';

test('inferred resource bindings preserve own/borrow identity and drop once',{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-inferred-resources-'));
  try {
    await build('examples/components/resources.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    let closed=0;const memories=[];
    class File {
      constructor(){this.value=42;this.closed=false;}
      [Symbol.dispose](){assert.equal(this.closed,false);this.closed=true;closed++;}
    }
    const host={File,make:()=>new File(),read:f=>{assert.equal(f.closed,false);return f.value;}};
    const {api}=await instantiate({imports:{'test:resources/host':host},instantiateCore:async(m,i)=>{
      const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;
    }});
    for(let i=0;i<20;i++) {
      const file=await api.make();assert.ok(file instanceof File);
      assert.equal(await api.read(file),42);assert.equal(closed,i,'borrowing must leave the owner alive');
      assert.equal(await api.echo(file),file);await api.close(file);assert.equal(closed,i+1);
    }
    const file=await api.make();assert.equal(await api.optional(file),file);assert.equal(closed,20);
    await api.releaseOption(file);await api.releaseOption(undefined);assert.equal(closed,21);
    assert.equal(await api.handled(),42);assert.equal(closed,22,'handled exceptions close the scoped owner once');
    assert.ok(memories.length);for(const m of memories)assert.equal(m.live(),0);
    if(global.gc) {
      const weak=await(async()=>{const f=await api.make();const weak=new WeakRef(f);await api.close(f);return weak;})();
      await collect();assert.equal(weak.deref(),undefined,'closed host resource retained');
    }
    // Separate instantiations use their own resource tables.
    const other=await instantiate({imports:{'test:resources/host':host}});
    const otherFile=await other.api.make();await other.api.close(otherFile);assert.equal(otherFile.closed,true);
  } finally {await rm(dir,{recursive:true,force:true});}
});

test('private component resource providers destroy scoped owners without exporting handles',{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-private-resources-'));
  try {
    await build('tests/component-fixtures/composition-private-resources.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    let memory;
    const {api}=await instantiate({instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memory=x.exports;return x;}});
    for(let n=0;n<100;n++) {
      assert.equal(await api.run(),42);assert.equal(await api.closedCount(),n+1);assert.equal(memory.live(),0);
    }
    const fresh=await instantiate();assert.equal(await fresh.api.closedCount(),0);
    assert.equal(await fresh.api.run(),42);assert.equal(await fresh.api.closedCount(),1);
    assert.equal(await api.closedCount(),100);
  } finally {await rm(dir,{recursive:true,force:true});}
});
