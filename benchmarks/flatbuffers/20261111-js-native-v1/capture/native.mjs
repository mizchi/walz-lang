import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {performance} from 'node:perf_hooks';
import {cpus} from 'node:os';
import {createReference} from './reference.mjs';
import {materialize,hash} from './workloads.mjs';
const median=xs=>[...xs].sort((a,b)=>a-b)[Math.floor(xs.length/2)];
let sink;
export async function run(directory,output,{rounds=9}={}){
 assert.equal(typeof global.gc,'function');
 const reference=createReference(await import(pathToFileURL(join(directory,'reference-package/js/flatbuffers.js'))));
 const programs={};
 for(const name of ['core','component','direct-utf8','direct-js-strings']){
  const {instantiate}=await import(pathToFileURL(join(directory,name,'index.js')));
  let arena;programs[name]=await instantiate({instantiateCore:async(m,i)=>{const instance=await WebAssembly.instantiate(m,i);if(instance.exports.live)arena=instance.exports;return instance;}});
  programs[name].clean=()=>{if(arena)assert.equal(arena.live(),0);};
 }
 const jobs=[];
 for(const c of JSON.parse(await readFile(join(directory,'corpus.json'),'utf8'))){
  const value=materialize(c.value),bytes=new Uint8Array(await readFile(join(directory,'inputs',c.name+'.bin')));
  const checkBytes=b=>assert.deepEqual(reference.decode(b),value);
  const reused=reference.createEncoder();
  jobs.push({name:c.name+'.encode.js-fresh',fn:()=>reference.encode(value),check:checkBytes},
   {name:c.name+'.encode.js-reused',fn:()=>reused(value),check:checkBytes},
   {name:c.name+'.decode.js',fn:()=>reference.decode(bytes),check:m=>assert.deepEqual(m,value)});
  for(const [name,program]of Object.entries(programs)){
   const clean=()=>{program.clean();const memory=program.__memory?.();if(memory){if(name.startsWith('direct-')){assert.equal(memory.hostHandles,0);assert.equal(memory.boundaryBuffers,0);assert.ok(memory.scratchCapacityBytes<=16777216);}else assert.ok(Object.values(memory).every(n=>n===0));}};
   jobs.push({name:c.name+'.encode.'+name,fn:()=>program.api.encode(value),check:checkBytes,clean},
    {name:c.name+'.decode.'+name,fn:()=>program.api.decode(bytes),check:m=>assert.deepEqual(m,{tag:'ok',val:value}),clean});
  }
 }
 for(const name of ['text-core','text-utf8','text-js-strings']){
  const {instantiate}=await import(pathToFileURL(join(directory,name,'index.js'))),{api}=await instantiate();
  for(const [size,input]of [['small','A😀'.repeat(4)],['64k','A😀'.repeat(13107)]])
   for(const op of ['echo','length'])jobs.push({name:`text-${size}.${op}.${name}`,fn:()=>api[op](input),check:v=>assert.equal(v,op==='echo'?input:new TextEncoder().encode(input).length)});
 }
 const measure=async(job,n)=>{const start=performance.now();for(let i=0;i<n;i++)sink=job.fn();const elapsed=(performance.now()-start)*1e6;job.check(sink);await new Promise(resolve=>setImmediate(resolve));job.clean?.();return elapsed/n;};
 for(const job of jobs){
  job.check(job.fn());job.clean?.();let calls=0;const start=performance.now();
  do{await measure(job,64);calls+=64;}while(calls<4096||performance.now()-start<250);
  job.calls=1;while(job.calls<(1<<20)&&(await measure(job,job.calls))*job.calls<20e6)job.calls*=2;
  job.samples=[];
 }
 for(let round=0;round<rounds;round++)for(const job of round%2?[...jobs].reverse():jobs){global.gc();job.samples.push(await measure(job,job.calls));}
 const report={schema:'walz-flatbuffers-js-native-v1',created:new Date().toISOString(),node:process.version,v8:process.versions.v8,cpu:cpus()[0]?.model,rounds,
  captureHash:hash(await readFile(join(directory,'capture.json'))),method:'Captured identical input/reference. Warmup >=4096 calls and 250ms per job, 20ms calibration, alternate job order by round, explicit GC before samples; automatic GC inside clocks counts. Full results, scratch bounds and event-loop drain outside clocks. Native profiles intentionally omit queued start/cancellation/host resources and keep shared-value checks. Text echo/length uses reused valid input; JS string mode defers UTF-8 conversion until needed. No profiler or build/test activity during timing.',
  cases:jobs.map(j=>({name:j.name,calls:j.calls,samplesNs:j.samples,medianNs:median(j.samples)}))};
 await mkdir(output);await writeFile(join(output,'native.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
 for(const c of report.cases)console.log(c.name+': '+(c.medianNs/1000).toFixed(3)+' µs');return report;
}
