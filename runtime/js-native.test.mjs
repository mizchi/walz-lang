import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import * as fb from 'flatbuffers';
import {createReference} from './flatbuffers-bench/reference.mjs';
import {build} from './build-js-native.mjs';
const reference=createReference(fb);
function decodePreservingBOM(bytes){
 const m=reference.decode(bytes),b=new fb.ByteBuffer(bytes),t=b.readUint32(0);
 const text=(t,slot)=>{const off=b.__offset(t,4+slot*2);return off?new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(b.__string(t+off,fb.Encoding.UTF8_BYTES)):'';};
 m.name=text(t,1);const child=b.__offset(t,16);if(child)m.child.label=text(b.__indirect(t+child),1);return m;
}
for(const mode of ['utf8','js-strings'])test(`experimental ${mode}: native values preserve UTF-8, bounds, snapshots and Result errors`,{timeout:120000},async()=>{
 const root=await mkdtemp(join(tmpdir(),'walz-js-native-'));
 try{
  await build('examples/flatbuffers/main.walz',join(root,'fb'),{mode});
  const {instantiate}=await import(pathToFileURL(join(root,'fb/index.js')));
  const program=await instantiate(),{api}=program;
  assert.equal(api.main(),42);assert.equal(program.start,undefined);
  const m={id:-9223372036854775808n,name:'\ufeff日本語🦀\0',values:Int32Array.of(-2147483648,-1,0,2147483647),
    payload:Uint8Array.of(0,128,255),enabled:false,weight:-1.25,child:{count:42,label:'child🦀'}};
  for(const n of [0,1,17,65537]){
   const value={...m,payload:Uint8Array.from({length:n},(_,i)=>i)};
   assert.deepEqual(decodePreservingBOM(api.encode(value)),value);
   assert.deepEqual(api.decode(reference.encode(value)),{tag:'ok',val:value});
  }
  assert.deepEqual(api.decode(new Uint8Array()),{tag:'err',val:1});
  assert.ok(Object.is(api.decode(api.encode({...m,weight:-0})).val.weight,-0));
  const bad=reference.encode(m),b=new fb.ByteBuffer(bad),t=b.readUint32(0),s=b.__indirect(t+b.__offset(t,6));
  bad[s+4]=255;assert.deepEqual(api.decode(bad),{tag:'err',val:4});
  for(let i=0;i<100;i++){assert.deepEqual(api.decode(api.encode(m)),{tag:'ok',val:m});api.decode(bad);}
  const capacity=program.__memory().scratchCapacityBytes;
  for(let i=0;i<100;i++)api.decode(api.encode(m));
  assert.equal(program.__memory().scratchCapacityBytes,capacity,'scratch stays bounded and reusable');
  for(const wrong of [{...m,id:1},{...m,enabled:1},{...m,values:[2147483648]},{...m,name:42}])assert.throws(()=>api.encode(wrong),TypeError);
  assert.equal(api.main(),42,'argument validation does not poison the guest');
  const reentrant={...m,get id(){assert.throws(()=>api.main(),/busy/);return m.id;}};
  assert.deepEqual(api.decode(api.encode(reentrant)),{tag:'ok',val:m});
  assert.deepEqual(api.decode(api.encode({...m,name:'\ud800'})).val.name,'�','JS input canonicalization matches TextEncoder');
  await build('tests/component-fixtures/js-native.walz',join(root,'values'),{mode});
  const {instantiate:values}=await import(pathToFileURL(join(root,'values/index.js')));
  const {api:v}=await values();
  assert.equal(v.echo('A😀'),'A😀');assert.equal(v.length('A😀'),5);
  assert.equal(v.concat('A😀','\ufeffB'),'A😀\ufeffB');
  assert.equal(v.literal(),'\ufeff日本🦀');
  assert.equal(v.fromBytes(Uint8Array.of(255)),undefined);
  assert.equal(v.fromBytes(Uint8Array.of(239,187,191,65)),'\ufeffA');
  const input=Uint8Array.of(1,2),out=v.bytes(input);input[0]=9;assert.deepEqual(out,Uint8Array.of(1,2));
  assert.deepEqual(v.longs([-9223372036854775808n,9223372036854775807n]),BigInt64Array.of(-9223372036854775808n,9223372036854775807n));
  assert.deepEqual(v.floats([1.1,-0,NaN]),Float32Array.of(1.1,-0,NaN));
  assert.deepEqual(v.nested([[1,2],[],[-1]]),[Int32Array.of(1,2),new Int32Array(),Int32Array.of(-1)]);
  for(const option of [{tag:'none'},{tag:'some',val:undefined},{tag:'some',val:'🦀'}])assert.deepEqual(v.options(option),option);
  for(const unit of [['value',undefined],['value','value']])assert.deepEqual(v.units(unit),unit);
  assert.deepEqual(v.unitList(['value','value']),['value','value']);
  const {api:limits}=await values();
  assert.throws(()=>limits.lengthCatch('a'.repeat(16777217)),RangeError,'host failures cannot become successful catch_all values');
  assert.throws(()=>v.trap(),WebAssembly.RuntimeError);assert.throws(()=>v.echo('x'),/failed|failure/);
  if(global.gc)for(const invalidArgument of [false,true]){
   const history=await(async()=>{
    let memory;
    const {api}=await values({instantiateCore:async(m,i)=>{const instance=await WebAssembly.instantiate(m,i);memory=new WeakRef(instance.exports.__transfer);return instance;}});
    let error;try{if(invalidArgument)api.length(42);else api.trap();}catch(e){error=e;}
    return {memory,error};
   })();
   let collected=false;
   for(let i=0;i<30;i++){
    await new Promise(r=>setImmediate(r));global.gc();await new Promise(r=>setImmediate(r));
    if(!history.memory.deref()){collected=true;break;}
   }
   assert.ok(collected,'keeping a failed-call error must not retain its guest memory');
   assert.ok(history.error instanceof (invalidArgument?TypeError:WebAssembly.RuntimeError));
  }
 }finally{await rm(root,{recursive:true,force:true});}
});
