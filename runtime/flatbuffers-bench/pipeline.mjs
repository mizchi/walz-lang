import assert from "node:assert/strict";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";
import { cpus } from "node:os";
import { createReference } from "./reference.mjs";
import { materialize, hash } from "./workloads.mjs";

export function sumI32(values) {
  let sum=0n,partial=0;
  for(let i=0;i<values.length;i++) {
    partial+=values[i];
    if((i&1048575)===1048575){sum+=BigInt(partial);partial=0;}
  }
  return sum+BigInt(partial);
}
export const summarize = m => ({id:m.id,name:m.name,count:m.values.length,
  sum:sumI32(m.values),size:m.payload.length,
  enabled:m.enabled,weight:m.weight,child:m.child?.count});
export function transform(m,delta) {
  const values=new Int32Array(m.values.length);
  for(let i=0;i<values.length;i++)values[i]=m.values[i]+delta;
  return {...m,values};
}
const littleEndian=new Uint8Array(Uint32Array.of(1).buffer)[0]===1;
// Official ByteBuffer accessors expect trusted input; this is not a verifier.
export function query(ByteBuffer, bytes) {
  const b = new ByteBuffer(bytes), t = b.readUint32(0);
  const field = (t,slot) => {const o=b.__offset(t,4+slot*2);return o?t+o:0;};
  const read = (slot,fn,fallback) => {const p=field(t,slot);return p?fn.call(b,p):fallback;};
  const text = (t,slot) => {const p=field(t,slot);return p?b.__string(p):"";};
  const p=field(t,2), count=p?b.__vector_len(p):0, start=p?b.__vector(p):0;
  let sum;
  if(littleEndian&&(bytes.byteOffset+start)%4===0) {
    sum=sumI32(new Int32Array(bytes.buffer,bytes.byteOffset+start,count));
  } else {
    sum=0n;let partial=0;
    for(let i=0;i<count;i++) {
      partial+=b.readInt32(start+i*4);
      if((i&1048575)===1048575){sum+=BigInt(partial);partial=0;}
    }
    sum+=BigInt(partial);
  }
  const payload=field(t,3), childSlot=field(t,6), child=childSlot?b.__indirect(childSlot):0;
  if(child)text(child,1);
  const childCount=child?field(child,0):0;
  return {id:read(0,b.readInt64,0n),name:text(t,1),count,sum,
    size:payload?b.__vector_len(payload):0,enabled:Boolean(read(4,b.readUint8,1)),
    weight:read(5,b.readFloat64,0),child:child?(childCount?b.readInt32(childCount):0):undefined};
}
const median = xs => [...xs].sort((a,b)=>a-b)[Math.floor(xs.length/2)];
let sink;
export async function run(directory, output, {rounds=7,batchSize=8}={}) {
  assert.equal(typeof global.gc,"function","run with node --expose-gc");
  assert.ok(Number.isInteger(rounds)&&rounds>=5&&rounds<=31);
  assert.ok(Number.isInteger(batchSize)&&batchSize>0&&batchSize<=64);
  const fb=await import(pathToFileURL(join(directory,"reference-package/js/flatbuffers.js")));
  const reference=createReference(fb), adapters={};
  for(const backend of ["core","component"]) {
    const {instantiate}=await import(pathToFileURL(join(directory,"pipeline-"+backend,"index.js")));
    let arena;
    const program=await instantiate({instantiateCore:async(m,i)=>{
      const instance=await WebAssembly.instantiate(m,i);if(instance.exports.live)arena=instance.exports;return instance;
    }});
    adapters[backend]={api:program.api,clean(){assert.equal(arena.live(),0);assert.ok(Object.values(program.__memory()).every(n=>n===0));}};
  }
  const jobs=[];
  for(const c of JSON.parse(await readFile(join(directory,"corpus.json"),"utf8")).filter(c=>c.name!=="unicode-4k")) {
    const value=materialize(c.value), bytes=new Uint8Array(await readFile(join(directory,"inputs",c.name+".bin")));
    const inputs=Array.from({length:batchSize},()=>bytes), expected=summarize(value), changed=transform(value,1);
    const checkSummary=xs=>{assert.equal(xs.length,batchSize);for(const x of xs)assert.deepEqual(x,expected);};
    const checkBytes=xs=>{assert.equal(xs.length,batchSize);for(const x of xs)assert.deepEqual(reference.decode(x),changed);};
    const add=(name,fn,check,clean)=>jobs.push({name:c.name+"."+name,fn,check,clean});
    add("inspect.js-query",()=>inputs.map(b=>query(fb.ByteBuffer,b)),checkSummary);
    add("transform.js-materialized",()=>inputs.map(b=>reference.encode(transform(reference.decode(b),1))),checkBytes);
    const reused=reference.createEncoder();
    add("transform.js-reused",()=>inputs.map(b=>reused(transform(reference.decode(b),1))),checkBytes);
    const unwrap=r=>{assert.equal(r.tag,"ok");return r.val;};
    for(const [backend,{api,clean}] of Object.entries(adapters)) {
      add("inspect."+backend+"-materialized",()=>inputs.map(b=>summarize(unwrap(api.decode(b)))),checkSummary,clean);
      add("inspect."+backend+"-query",()=>inputs.map(b=>unwrap(api.inspect(b))),checkSummary,clean);
      add("inspect."+backend+"-batch",()=>api.inspectBatch(inputs).map(unwrap),checkSummary,clean);
      add("transform."+backend+"-materialized",()=>inputs.map(b=>api.encode(transform(unwrap(api.decode(b)),1))),checkBytes,clean);
      add("transform."+backend+"-fused",()=>inputs.map(b=>unwrap(api.transform(b,1))),checkBytes,clean);
      add("transform."+backend+"-batch",()=>api.transformBatch(inputs,1).map(unwrap),checkBytes,clean);
    }
  }
  const measure=async(job,calls)=>{
    const start=performance.now();for(let i=0;i<calls;i++)sink=job.fn();
    const elapsed=(performance.now()-start)*1e6;
    job.check(sink);await new Promise(resolve=>setImmediate(resolve));job.clean?.();return elapsed/calls/batchSize;
  };
  for(const job of jobs) {
    job.check(job.fn());job.clean?.();
    const start=performance.now();let calls=0;
    do{await measure(job,32);calls+=32;}while(calls<512||performance.now()-start<250);
    job.calls=1;while(job.calls<65536&&(await measure(job,job.calls))*job.calls*batchSize<20e6)job.calls*=2;
    job.samples=[];
  }
  for(let round=0;round<rounds;round++)for(const job of round%2?[...jobs].reverse():jobs) {
    global.gc();job.samples.push(await measure(job,job.calls));
  }
  const report={schema:"walz-flatbuffers-pipeline-v2",created:new Date().toISOString(),node:process.version,v8:process.versions.v8,cpu:cpus()[0]?.model,
    captureHash:hash(await readFile(join(directory,"capture.json"))),rounds,batchSize,
    method:"Same valid input, full output checked outside clocks. Times per message; batch=8 by default. Warmup >=512 batches (4096 messages at batch=8) and 250ms, 20ms calibration. Alternate job order each round, GC before each sample, event-loop drain and ABI cleanup checks outside clocks. JS query uses official ByteBuffer accessors and a zero-copy Int32Array view on trusted aligned little-endian input (bytewise fallback otherwise); Walz also validates every known field. JS transform uses a manual typed-array loop, with fresh/reused official builders, always returning owned bytes. Transform is decode/map/re-encode everywhere; i32 addition wraps. Setup excluded; ABI transfers, lifting, JS arithmetic and Wasm work included. v1's TypedArray.map and bytewise query were replaced; v1 evidence is retained separately.",
    cases:jobs.map(j=>({name:j.name,calls:j.calls,samplesNs:j.samples,medianNs:median(j.samples)}))};
  await mkdir(output);await writeFile(join(output,"pipeline.json"),JSON.stringify(report,null,2)+"\n",{flag:"wx"});
  for(const c of report.cases)console.log(`${c.name}: ${(c.medianNs/1000).toFixed(2)} µs/message`);
  return report;
}
