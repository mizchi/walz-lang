import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {build} from './build.mjs';

test('generic constructors preserve value ABIs, endpoint moves and independent state',{timeout:120000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-component-generics-'));
  try {
    await build('examples/components/generics.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    const memories=[];
    const {api}=await instantiate({instantiateCore:async(m,i)=>{
      const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;
    }});
    assert.equal(api.narrow.echo(42),42);
    assert.equal(api.wide.echo(9007199254740993n),9007199254740993n);
    assert.deepEqual([api.narrow.next(),api.narrow.next(),api.again.next(),api.wide.next()],[1,2,1,1]);
    for(let i=0;i<20;i++) {
      const text='日本😀'.repeat(i*512),values=Int32Array.from({length:i*4096},(_,j)=>j-42);
      assert.equal(api.text.echo(text),text);
      assert.deepEqual(api.values.echo(values),values);
      assert.equal(api.values.echo(undefined),undefined);
      assert.equal(await (await api.pending.transfer(Promise.resolve(42))),42);
    }
    assert.equal(await api.wait.consume(new Promise(r=>setTimeout(()=>r(42),10))),42);
    await new Promise(r=>setTimeout(r,25));
    assert.ok(memories.length>0,'observe the emitted ABI arenas');
    for(const m of memories)assert.equal(m.live(),0,'retained canonical buffer');
    assert.equal((await instantiate()).api.narrow.next(),1);
    // A rejected native operation belongs to an isolated instance/store.
    const failed=await instantiate();
    await assert.rejects(failed.api.wait.consume(Promise.reject(new Error('host failure'))));
    const fresh=await instantiate();
    assert.equal(await fresh.api.wait.consume(Promise.resolve(42)),42);
  } finally {await rm(dir,{recursive:true,force:true});}
});
