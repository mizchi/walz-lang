import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {instrument,collect} from './testing/memory.mjs';
import {createEventLoop} from './host.mjs';
import {withTaskScope} from './scopes.mjs';

for(const backend of ['core','component'])test(`${backend}: deadline scopes wait for child disposal and isolate other calls`,{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-deadlines-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/deadlines.wlt',dir);await instrument(dir,backend);
    const runtime=await import(pathToFileURL(join(dir,'index.js')));
    assert.equal(typeof runtime.withTaskScope,'function');
    let disposed=0,started=0,guest,allocator;
    const component=await runtime.instantiate({eventLoop:{...createEventLoop(),sleep:ms=>{
      if(!ms)return Promise.resolve();
      started++;const done=Promise.withResolvers();done.promise[Symbol.asyncDispose]=async()=>{await new Promise(r=>setTimeout(r,5));disposed++;done.resolve();};return done.promise;
    }},instantiateCore:async(module,imports)=>{const i=await WebAssembly.instantiate(module,imports);if(i.exports.live)allocator=i.exports;if(i.exports.__children_live)guest=i.exports;return i;}});
    const {api,start}=component;
    for(const timeoutMs of [-1,NaN,Infinity,2147483648])assert.throws(()=>start('value',[],{timeoutMs}),/timeout/);
    const queued=start('children',[1000],{timeoutMs:0});
    await assert.rejects(queued.result,e=>e.name==='TaskTimeoutError');assert.equal(started,0);
    const task=start('children',[1000],{timeoutMs:100});
    const unrelated=start('value');
    await assert.rejects(task.result,e=>e.name==='TaskTimeoutError'&&e instanceof runtime.TaskCancelledError);
    assert.equal(disposed,2);assert.equal(await unrelated.result,42);
    assert.equal(guest.__children_live(),0);assert.equal(allocator.live(),0);
    assert.ok(Object.values(component.__memory()).every(n=>n===0),JSON.stringify(component.__memory()));
    const scope=withTaskScope(start,async group=>{
      const pending=group.start('children',[1000]);
      group.start('value');
      return pending.result;
    },{timeoutMs:100});
    const outside=start('value');
    await assert.rejects(scope,e=>e.name==='TaskTimeoutError');
    assert.equal(await outside.result,42);assert.equal(disposed,4);
    await assert.rejects(withTaskScope(start,async group=>{group.start('children',[1000]);await Promise.resolve();throw new Error('callback failed');}),/callback failed/);
    assert.equal(allocator.live(),0);assert.equal(guest.__children_live(),0);
    assert.equal(await api.children(0),42);
    await assert.rejects(withTaskScope(start,async()=>{throw 0;}),reason=>reason===0);
    let escaped;
    assert.equal(await withTaskScope(start,async group=>{escaped=group;return group.start('value').result;}),42);
    assert.throws(()=>escaped.start('value'),/closed/);
    const events=[];const timers=await runtime.instantiate({eventLoop:createEventLoop({onEvent:e=>events.push(e)})});
    await timers.api.timer(0);await timers.start('closeTimer',[60000]).result;
    assert.ok(events.some(e=>e.type==='dispose'&&e.milliseconds===60000));
    assert.equal(await timers.api.value(),42);
    if(typeof global.gc==='function'){
      const history=await(async()=>{
        let memory,expired;
        const {start}=await runtime.instantiate({instantiateCore:async(m,imports)=>{const i=await WebAssembly.instantiate(m,imports);if(i.exports.memory)memory=i.exports.memory;return i;}});
        await runtime.withTaskScope(start,async scope=>{expired=scope;return scope.start('value').result;});
        return {expired,ref:new WeakRef(memory)};
      })();
      await new Promise(r=>setTimeout(r,25));await collect();
      assert.equal(history.expired.signal,undefined);assert.equal(history.ref.deref(),undefined,'closed host scope retained the instance');
    }
  }finally{await rm(dir,{recursive:true,force:true});}
});
