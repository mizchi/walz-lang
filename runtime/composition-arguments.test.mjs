import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {build} from './build.mjs';

test('staged nominal constructors preserve record, future and stream values without retaining buffers',{timeout:120000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-typed-constructors-'));
  try {
    await build('examples/components/typed-constructors.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    const memories=[];
    const options={imports:{'test:constructors/host':{}},instantiateCore:async(m,i)=>{
      const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;
    }};
    const {api}=await instantiate(options);
    for(let n=0;n<12;n++) {
      const item={value:4294967295,label:'日本😀'.repeat(n*1024)};
      for(const name of ['first','again','relay']) {
        assert.deepEqual(api[name].echo(item),item);
        assert.deepEqual(await(await api[name].ready(item)),item);
        const stream=await api[name].stream(item),values=[];
        for await(const value of stream)values.push(value);
        assert.deepEqual(values,[item]);
      }
      const future=await api.future.transfer(new Promise(r=>setTimeout(()=>r(item),1)));
      assert.deepEqual(await future,item);
      const stream=await api.stream.transfer((async function*(){yield item;yield item;})()),values=[];
      for await(const value of stream)values.push(value);
      assert.deepEqual(values,[item,item]);
    }
    await new Promise(r=>setTimeout(r,25));
    assert.ok(memories.length>0);
    for(const m of memories)assert.equal(m.live(),0,'retained canonical buffer after normal completion');
    // A rejected producer may poison its native store. It is isolated from
    // normal-completion arena measurements and is never reused or free-all'd.
    const failed=await instantiate({imports:options.imports});
    await assert.rejects(failed.api.await.consume(Promise.reject(new Error('failed producer'))));
    const fresh=await instantiate(options),item={value:42,label:'fresh'};
    assert.deepEqual(await fresh.api.await.consume(Promise.resolve(item)),item);
    // Closing an iterator follows native stream lifetime; no pending
    // canonical allocation is freed speculatively on a cancellation request.
    const closed=await api.first.stream(item);
    const iterator=closed[Symbol.asyncIterator]();
    assert.deepEqual((await iterator.next()).value,item);
    await iterator.return();
    await new Promise(r=>setTimeout(r,25));
    for(const m of memories)assert.equal(m.live(),0,'retained canonical buffer after close');
  } finally {await rm(dir,{recursive:true,force:true});}
});

test('value type export ascriptions keep resource aliases and scoped exception cleanup',{timeout:120000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-nominal-resources-'));
  try {
    await build('tests/component-fixtures/composition-nominal-resources.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));let closed=0;
    class File {constructor(){this.closed=false;}[Symbol.dispose](){assert.equal(this.closed,false);this.closed=true;closed++;}}
    const memories=[];
    const {api}=await instantiate({imports:{'test:resources/host':{File,make:()=>new File(),read:f=>{assert.equal(f.closed,false);return 42;}}},
      instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;}});
    for(let n=0;n<12;n++) {
      assert.deepEqual(api.left.echo({value:42}),{value:42});
      const file=await api.left.make();
      assert.equal(await api.right.read(file),42,'resource keeps the actual shared host identity');
      assert.equal(closed,n*2);await api.right.close(file);
      assert.equal(await api.left.handled(),42);assert.equal(closed,(n+1)*2);
    }
    assert.ok(memories.length>0);for(const m of memories)assert.equal(m.live(),0);
  } finally {await rm(dir,{recursive:true,force:true});}
});
