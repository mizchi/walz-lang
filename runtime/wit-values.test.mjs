import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';

const clean=v=>ArrayBuffer.isView(v)?Array.from(v):Array.isArray(v)?v.map(clean):v&&typeof v==='object'?Object.fromEntries(Object.entries(v).filter(([,v])=>v!==undefined).map(([k,v])=>[k,clean(v)])):v;
for(const backend of ['core','component'])test(`${backend}: WIT scalars, tuples and joined variant payloads round trip and release`,{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-wit-values-'));
  try{
    let source=(await readFile('tests/component-fixtures/wit-values.wlt','utf8')).trimEnd();
    source=source.slice(0,-1)+`
      (import "host" "echo" (func $echo (param $v (ref $Value)) (result (ref $Value))))
      (func $forward (export "forward") (param $v (ref $Value)) (result (ref $Value)) (%effect host) (call $echo $v))
      (func $bad (export "bad-bool") (result $Bool) (%effect pure) (%newtype.wrap $Bool 2)))`;
    const shape='(variant (empty) (text text) (number f64) (items (list i32)))';
    const policy={imports:[{module:'host',name:'echo',params:[shape],result:shape,suspends:false,capability:'echo'}]};
    const input=join(dir,'input.wlt'),bindings=join(dir,'policy.json');
    await writeFile(input,source);await writeFile(bindings,JSON.stringify(policy));
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build(input,dir,{bindings});
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    let allocator;
    const {api}=await instantiate({imports:{host:{echo:v=>v}},instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)allocator=instance.exports;return instance;
    }});
    assert.equal(api.boolean(true),true);assert.equal(api.boolean(false),false);
    for(const char of ['\0','A','漢','😀','\u{10ffff}'])assert.equal(api.character(char),char);
    for(const n of [-128,-1,0,127])assert.equal(api.signed8(n),n);
    for(const n of [-32768,0,32767])assert.equal(api.signed16(n),n);
    assert.deepEqual(api.triple([true,-128,'😀']),[true,-128,'😀']);
    const cases=[{tag:'empty'},{tag:'text',val:'漢😀\0'},{tag:'number',val:-42.5},{tag:'items',val:[1,-2,3]}];
    for(let i=0;i<50;i++)for(const v of cases){assert.deepEqual(clean(api.value(v)),v);assert.deepEqual(clean(api.forward(v)),v);}
    assert.deepEqual(clean(api.values(cases)),cases);
    assert.throws(()=>api.value({tag:'unknown'}));
    if(backend==='core'){
      for(const char of ['', 'ab','\ud800'])assert.throws(()=>api.character(char),/char/);
      assert.throws(()=>api.boolean(1),/bool/);assert.throws(()=>api.signed8(128),/s8/);
    }
    assert.throws(()=>api.badBool());
    assert.equal(allocator.live(),0);
  }finally{await rm(dir,{recursive:true,force:true});}
});
