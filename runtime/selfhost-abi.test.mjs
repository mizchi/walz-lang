import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {programs,invalid,projects,badProjects,unsupported,diamond} from './selfhost-front/abi.mjs';
import {instrument,collect} from './testing/memory.mjs';
const bytes=s=>new TextEncoder().encode(s);
const files=p=>p.map(([path,source])=>({path,source:bytes(source)}));
for(const backend of ['core','component'])test(`${backend}: checked ABI metadata preserves Core validation`,{timeout:180000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-selfhost-abi-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    const inspection=join(directory,'inspection');await build('tests/component-fixtures/selfhost-abi.walz',inspection);
    await instrument(inspection,backend);let metaArena;
    const metaValue=await(await import(pathToFileURL(join(inspection,'index.js')))).instantiate({instantiateCore:async(m,imports)=>{
      const instance=await WebAssembly.instantiate(m,imports);if(instance.exports.live)metaArena=instance.exports;return instance;
    }});
    const meta=metaValue.api,mclean=()=>{assert.equal(metaArena.live(),0);assert.ok(Object.values(metaValue.__memory()).every(n=>n===0));};
    const views=source=>{const r=meta.inspect(files([['main.walz',source]]));mclean();assert.equal(r.tag,'ok',JSON.stringify(r));return r.val;};
    const shape=({kind,shape,size,align,flat})=>({kind,shape,size,align,flat});
    assert.deepEqual(shape(views(programs[0][1])[0]),{kind:0,shape:'(record (x i32) (y i64))',size:16,align:8,flat:2});
    assert.deepEqual(shape(views(programs[1][1])[0]),{kind:1,shape:'(tuple i32 i64)',size:16,align:8,flat:2});
    assert.deepEqual(shape(views('(module (type $F (struct (field f32) (field f64))) (@abi.type $F (tuple)))')[0]),
      {kind:1,shape:'(tuple f32 f64)',size:16,align:8,flat:2},'metadata facts do not enable Core float instructions');
    assert.deepEqual(shape(views('(module (@use "walz:core" (type $unit)) (type $U (struct (field $unit))) (@abi.type $U (tuple)))')[0]),
      {kind:1,shape:'(tuple unit)',size:1,align:1,flat:1});
    assert.deepEqual(shape(views(programs[3][1])[0]),{kind:0,
      shape:'(record (blob bytes) (label text) (items (list i32)) (maybe (option i32)) (outcome (result i32 i64)) (pair (pair i32 i64)))',
      size:64,align:8,flat:12});
    assert.equal(views(diamond(10)).at(-1).flat,1024,'memoized diamond reaches the inclusive flat bound');
    await build('examples/selfhost/portable.walz',directory);await instrument(directory,backend);
    let arena;
    const value=await(await import(pathToFileURL(join(directory,'index.js')))).instantiate({instantiateCore:async(m,imports)=>{
      const instance=await WebAssembly.instantiate(m,imports);if(instance.exports.live)arena=instance.exports;return instance;
    }});
    const {api}=value,clean=()=>{assert.equal(arena.live(),0);assert.ok(Object.values(value.__memory()).every(n=>n===0));};
    for(const [name,source,expected] of programs){
      const r=api.compileProject(files([['main.walz',source]]));clean();
      assert.equal(r.tag,'ok',name+' '+JSON.stringify(r));assert.ok(WebAssembly.validate(r.val));
      assert.equal((await WebAssembly.instantiate(r.val)).instance.exports.f(),expected,name);
    }
    const annotated=programs[0][1],plain=annotated.replace(/\(@abi\.type \$Point \(record "x" "y"\)\)/,'');
    assert.deepEqual(api.compileProject(files([['main.walz',annotated]])).val,
      api.compileProject(files([['main.walz',plain]])).val,'checked views do not change Core instructions');clean();
    for(const [name,source,code] of invalid){
      const r=api.checkProject(files([['main.walz',source]]));clean();
      assert.equal(r.tag,'err',name);assert.equal(r.val.code,code,name);
      if(code===225||code===226)assert.match(new TextDecoder().decode(bytes(source).slice(r.val.start,r.val.end)),/^\(@abi\.type/,'ABI failures retain the annotation range');
      if(code===225||code===226){const metaResult=meta.inspect(files([['main.walz',source]]));mclean();assert.equal(metaResult.val.code,code,name);}
    }
    for(const [name,p,expected] of projects){
      const r=api.compileProject(files(p));clean();assert.equal(r.tag,'ok',name+' '+JSON.stringify(r));
      assert.ok(WebAssembly.validate(r.val));assert.equal((await WebAssembly.instantiate(r.val)).instance.exports.f(),expected);
    }
    for(const [name,p,code] of badProjects){
      const r=api.checkProject(files(p));clean();assert.equal(r.tag,'err',name);assert.equal(r.val.code,code,name);
      if(name==='abi-provider-range'){
        assert.equal(r.val.file,1);
        assert.equal(new TextDecoder().decode(bytes(p[1][1]).slice(r.val.start,r.val.end)),'(@abi.type $R (record "wrong"))');
      }
    }
    for(const [name,source,code] of unsupported){const r=api.checkProject(files([['main.walz',source]]));clean();assert.equal(r.tag,'err',name);assert.equal(r.val.code,code,name);}
    const again=api.compileProject(files([['main.walz',programs[0][1]]]));clean();
    assert.equal(again.tag,'ok','failed views do not poison the next request');
    assert.equal((await WebAssembly.instantiate(again.val)).instance.exports.f(),42);
    await collect();clean();mclean();
  }finally{await rm(directory,{recursive:true,force:true});}
});
