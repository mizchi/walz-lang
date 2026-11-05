import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';

export function corpus() {
  const values=['null','true','false','0','-0','9007199254740993','-9223372036854775809','1.25e-12','1E+400','"日本語🦀"','"\\uD83E\\uDD80"','"\\u0000\\b\\f\\n\\r\\t\\/\\\\\\\""','[]','{}','[null,1,true,["a"],{"b":false}]','{"x":1,"x":2,"__proto__":null}'];
  let seed=2026;
  const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed;};
  const gen=depth=>{
    switch(random()%(depth?6:4)){
      case 0:return null;case 1:return !!(random()&1);case 2:return (random()|0)/1000;
      case 3:return ['日本語','🦀','a\nb','\0','"\\',''][random()%6];
      case 4:return Array.from({length:random()%5},()=>gen(depth-1));
      default:return Object.fromEntries(Array.from({length:random()%5},(_,i)=>[`key${i}`,gen(depth-1)]));
    }
  };
  for(let i=0;i<128;i++)values.push(JSON.stringify(gen(4)));
  return values;
}

for(const backend of ['core','component'])test(`${backend}: JSON library matches an independent parser`,{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-json-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/json-value.wlt',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    let memory;
    const {api}=await instantiate({instantiateCore:async(module,imports)=>{const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)memory=instance.exports;return instance;}});
    for(const source of corpus()){
      const result=api.normalize(source);
      assert.equal(result.tag,'ok',`${source}: ${JSON.stringify(result)}`);
      assert.deepEqual(JSON.parse(result.val),JSON.parse(source),source);
      assert.equal(memory.live(),0);
    }
    assert.deepEqual(JSON.parse(api.constructed().val),{values:["日本語",null],n:125});
    assert.deepEqual(JSON.parse(api.builder().val),{nothing:null,n:125,values:[null,true]});
    assert.equal(api.normalize('9007199254740993').val,'9007199254740993');
    assert.equal(api.normalize('1.2300e+45').val,'1.2300e+45');
    assert.equal(api.get('{"x":1,"x":[null,"日本語"]}','x'),'[null,"日本語"]');
    assert.equal(api.get('{"x":null}','missing'),undefined);
    for(const source of ['', ' ', '01','-','+1','1.','1e+','NaN','Infinity','true false','[1,]','{"x":1,}','[1 2]','{x:1}','"\n"','"\\x20"','"\\uD800"','"\\uDC00"','"\\uD800\\u0041"','\ufeffnull','['.repeat(129)+'0'+']'.repeat(129),' '.repeat(65537)]){
      const result=api.normalize(source);assert.equal(result.tag,'err',source);
      assert.equal(memory.live(),0);
      assert.ok(Number.isInteger(result.val) && result.val>=0 && result.val<=new TextEncoder().encode(source).length);
    }
    assert.deepEqual(api.normalize('[1,]'),{tag:'err',val:3});
    assert.deepEqual(api.normalize('1e+'),{tag:'err',val:3});
    // No recursive traversal of siblings, even for wide arrays and objects.
    const wide=JSON.stringify(Array.from({length:4000},(_,i)=>i));
    assert.deepEqual(JSON.parse(api.normalize(wide).val),JSON.parse(wide));
    assert.equal(memory.live(),0);
  }finally{await rm(dir,{recursive:true,force:true});}
});
