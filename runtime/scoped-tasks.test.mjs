import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {instrument,collect} from './testing/memory.mjs';

const source=`(module
  (type $Box (struct (field i32)))
  (type $Mutable (struct (field (mut i32))))
  (import "wlt:host/event-loop@0.1.0" "sleep" (func $gate (param $n i32)))
  (%closure $Work (%capture $n i32) (result i64) (%effect host suspends)
    (block (result i64) (call $gate $n) (i64.const 21)))
  (%closure $Identity (%capture $value (ref $Box)) (result (ref $Box)) (%effect pure) $value)
  (%closure $Bad (result i64) (%effect host suspends) (block (result i64) (call $gate 2) (unreachable)))
  (func $concurrent (export "concurrent") (result i64) (%effect host reads writes allocates suspends)
    (%task.scope (result i64)
      (%let $a (%task.spawn i64 $Work (%closure.new $Work 0))
        (%let $b (%task.spawn i64 $Work (%closure.new $Work 1))
          (+ (%task.join i64 (%move $a)) (%task.join i64 (%move $b)))))))
  (func $identity (export "identity") (result i32) (%effect host reads writes allocates suspends)
    (%let $object (struct.new $Box 42)
      (%let $task (%task.spawn (ref $Box) $Identity (%closure.new $Identity $object))
        (ref.eq $object (%task.join (ref $Box) (%move $task))))))
  (func $failure (export "failure") (result i64) (%effect host reads writes allocates suspends)
    (%task.scope (result i64)
      (%let $waiting (%task.spawn i64 $Work (%closure.new $Work 0))
        (%let $bad (%task.spawn i64 $Bad (%closure.new $Bad))
          (block (result i64) (%task.drop i64 (%move $waiting)) (%task.join i64 (%move $bad)))))))
  (%closure $Write (%capture $cell (ref $Mutable)) (%effect host writes suspends)
    (block (call $gate 3) (struct.set $Mutable 0 $cell (i32.const 42))))
  (func $make (param $cell (ref $Mutable)) (result (%task %unit)) (%effect host reads writes allocates suspends)
    (%task.scope (result (%task %unit))
      (return (%task.spawn %unit $Write (%closure.new $Write $cell)))))
  (func $early (export "early") (result i32) (%effect host reads writes allocates suspends)
    (%let $cell (struct.new $Mutable 0)
      (%let $task (call $make $cell)
        (%let $value (struct.get $Mutable 0 $cell)
          (block (result i32) (%task.drop %unit (%move $task)) $value)))))
  (func $trapped (export "trapped") (param $divisor i32) (%effect host reads writes allocates suspends)
    (%task.scope (result %unit)
      (%let $task (%task.spawn i64 $Work (%closure.new $Work 0))
        (block (drop (i32.div_s (i32.const 1) $divisor)) (%task.drop i64 (%move $task)))))))`;

for(const backend of ['core','component'])test(`${backend}: concurrent children, GC identity, cancellation, and scope cleanup`,{timeout:60000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'wlt-scoped-'));
  try {
    const input=join(directory,'main.wlt');await writeFile(input,source);
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build(input,join(directory,'js'));
    await instrument(join(directory,'js'),backend);
    const {instantiate,TaskCancelledError}=await import(pathToFileURL(join(directory,'js/index.js')));
    let mode='barrier',arrived=[],released=0,allocator,guest,started;
    const component=await instantiate({eventLoop:{sleep:n=>{
      const value=undefined;
      const done=Promise.withResolvers();arrived.push(done);
      done.promise[Symbol.asyncDispose]=()=>{released++;done.resolve(value);};
      if(mode==='ready' || mode==='failure' && n===2)done.resolve();
      if(mode==='barrier' && arrived.length===2)for(const item of arrived)item.resolve(value);
      if(arrived.length===2)started?.resolve();
      return done.promise;
    }},instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);
      if(instance.exports.live)allocator=instance.exports;
      if(instance.exports.__children_live)guest=instance.exports;
      return instance;
    }});
    assert.equal(component.wltChildRun,undefined,'private runner must not escape');
    const {api,start}=component;
    assert.equal(await api.concurrent(),42n,'both children must start before either finishes');
    assert.equal(arrived.length,2);assert.equal(await api.identity(),1,'GC result identity is preserved');
    assert.equal(allocator.live(),0);assert.equal(guest.__children_live(),0);
    assert.ok(Object.values(component.__memory()).every(n=>n===0),JSON.stringify(component.__memory()));
    for(let i=0;i<20;i++){arrived=[];assert.equal(await api.concurrent(),42n);assert.equal(guest.__children_live(),0);}
    mode='ready';arrived=[];assert.equal(await api.early(),42,'early return waits for the child before reading its result');
    mode='pending';arrived=[];started=Promise.withResolvers();
    const task=start('concurrent');await started.promise;
    await task.cancel('stop');await assert.rejects(task.result,TaskCancelledError);
    assert.equal(released,2);assert.equal(allocator.live(),0);assert.equal(guest.__children_live(),0);
    mode='failure';arrived=[];
    await assert.rejects(api.failure());
    assert.equal(arrived.length,2);
    assert.equal(released,3,'sibling failure disposes the waiting child');
    assert.equal(allocator.live(),0);assert.equal(guest.__children_live(),0);
    assert.equal(await api.identity(),1,'instance can be reused after failure');
    assert.ok(Object.values(component.__memory()).every(n=>n===0),JSON.stringify(component.__memory()));
    if(typeof global.gc==='function') {
      for(const mode of ['returned','cancelled','failed']) {
        const history=await(async()=>{
          let memory,count=0;const ready=Promise.withResolvers();
          const {start}=await instantiate({eventLoop:{sleep:n=>{
            const done=Promise.withResolvers();done.promise[Symbol.asyncDispose]=()=>done.resolve();
            if(mode==='failed' && n===2)done.resolve();
            if(++count===2)ready.resolve();return done.promise;
          }},instantiateCore:async(module,imports)=>{
            const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.memory)memory=instance.exports.memory;return instance;
          }});
          const task=start(mode==='returned'?'identity':mode==='failed'?'failure':'concurrent');
          if(mode==='cancelled'){await ready.promise;await task.cancel('stop');}
          await task.result.catch(()=>{});
          return {task,ref:new WeakRef(memory)};
        })();
        // The pinned jco scheduler retires its idle 10ms polling timer after
        // the task finishes. Let that bounded timer release its continuation.
        await new Promise(resolve=>setTimeout(resolve,25));
        await collect();assert.equal(history.ref.deref(),undefined,`${mode} child task history retained its store`);
      }
    }
    const trapped=await instantiate();await assert.rejects(trapped.api.trapped(0));
  }finally{await rm(directory,{recursive:true,force:true});}
});
