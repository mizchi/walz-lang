import test from 'node:test';
import assert from 'node:assert/strict';
import {assess} from './selfhost-memory/model.mjs';

test('retained-heap detector rejects growing heaps and backing stores',()=>{
  const samples=Array.from({length:8},(_,i)=>({heapUsed:4e6+i*512*1024,arrayBuffers:0,rss:100e6}));
  assert.equal(assess(samples).ok,false);
  assert.equal(assess(samples.map(s=>({...s,arrayBuffers:s.heapUsed,heapUsed:4e6}))).ok,false);
});

test('RSS high-water capacity is reported separately from retained live memory',()=>{
  const samples=Array.from({length:8},(_,i)=>({heapUsed:4e6+(i%2)*8192,arrayBuffers:65536,rss:100e6+i*8e6}));
  const result=assess(samples);
  assert.equal(result.ok,true);assert.equal(result.growth.rss,48e6);
  assert.throws(()=>assess(samples.slice(0,2)),/four/);
});

test('missing or invalid heap measurements cannot pass the audit',()=>{
  assert.throws(()=>assess(Array.from({length:4},()=>({rss:100e6}))),/heapUsed/);
  assert.throws(()=>assess(Array.from({length:4},()=>({heapUsed:NaN}))),/heapUsed/);
});

test('held public bridge errors release their compiler instance',{
  skip:typeof global.gc!=='function'||!process.env.WALZ_SELFHOST_MEMORY_COMPILER,
},async()=>{
  const {readFile}=await import('node:fs/promises');
  const {openStage}=await import('./selfhost/staged.mjs');
  const {installObserver}=await import('./selfhost-memory/observer.mjs');
  const {collect}=await import('./testing/memory.mjs');
  const observer=installObserver(),binary=await readFile(process.env.WALZ_SELFHOST_MEMORY_COMPILER);
  const errors=[];
  try{
    const kinds=['host-argument','wasm-trap','imports-argument','expand-argument','nested-error'];
    for(const [index,kind] of kinds.entries()){
      let api=await openStage(binary);
      if(kind==='wasm-trap')observer.failNextCompile();
      let expected;
      try{
        if(kind==='imports-argument')api.imports(null);
        else if(kind==='expand-argument')api.expandProject([{path:'main.walz',source:null}]);
        else if(kind==='nested-error')api.compileProject([{path:'main.walz',get source(){
          expected=new AggregateError([new Error('nested')],'outer',{cause:new Error('cause')});
          expected.cause.cause=expected;
          throw expected;
        }}]);
        else api.compileProject([{path:'main.walz',source:kind==='host-argument'?null:new Uint8Array()}]);
      }
      catch(error){errors.push(error);}
      assert.equal(errors.length,index+1);
      if(expected)assert.equal(errors.at(-1),expected,'preserve the original error and cause graph');
      api=null;await collect();
      assert.equal(observer.compilerAlive(),0,kind+' error must not retain the compiler');
      assert.deepEqual(observer.drain().live,{});
    }
    assert(errors[1] instanceof WebAssembly.RuntimeError,'fault injection must unwind actual Wasm');
  }finally{observer.restore();}
});
