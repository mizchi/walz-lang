import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {programs,nativePrograms,invalid,unsupported} from './selfhost-front/control.mjs';
import {instrument,collect} from './testing/memory.mjs';
const bytes=s=>new TextEncoder().encode(s);
for(const backend of ['core','component'])test(`${backend}: control contexts preserve unit matches and scalar conditions`,{timeout:180000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-selfhost-control-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('examples/selfhost/portable.walz',directory);await instrument(directory,backend);
    let arena;
    const value=await(await import(pathToFileURL(join(directory,'index.js')))).instantiate({instantiateCore:async(m,imports)=>{
      const instance=await WebAssembly.instantiate(m,imports);if(instance.exports.live)arena=instance.exports;return instance;
    }});
    const {api}=value,clean=()=>{assert.equal(arena.live(),0);assert.ok(Object.values(value.__memory()).every(n=>n===0));};
    const condition=programs.find(([name])=>name==='control-variable-condition')[1].replace(/\(@use "walz:core" \(type \$option\)\)/,'');
    const single=api.compile(bytes(condition));clean();assert.equal(single.tag,'ok','single-file shorthand selects the frontend');
    assert.equal((await WebAssembly.instantiate(single.val)).instance.exports.f(),42);
    assert.equal(api.check(bytes(condition)).tag,'ok');clean();
    for(const [name,source,expected] of [...programs,...nativePrograms]){
      const r=api.compileProject([{path:'main.walz',source:bytes(source)}]);clean();
      assert.equal(r.tag,'ok',name+' '+JSON.stringify(r));assert.ok(WebAssembly.validate(r.val),name);
      assert.equal((await WebAssembly.instantiate(r.val)).instance.exports.f(),expected,name);
    }
    for(const [name,source,code] of [...invalid,...unsupported]){
      const r=api.checkProject([{path:'main.walz',source:bytes(source)}]);clean();
      assert.equal(r.tag,'err',name);assert.equal(r.val.code,code,name);
      assert.ok(r.val.end>r.val.start,name+' retains its original range');
    }
    await collect();clean();
  }finally{await rm(directory,{recursive:true,force:true});}
});
