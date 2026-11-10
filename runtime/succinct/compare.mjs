import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const root=fileURLToPath(new URL('../../',import.meta.url));
const [before,after,id]=process.argv.slice(2);
for(const value of [before,after,id])if(!/^[a-zA-Z0-9][a-zA-Z0-9_-]{0,79}$/.test(value??''))throw new Error('usage: compare.mjs before after new-id');
const output=resolve(root,'benchmarks/succinct',id);await mkdir(output);
const runs=[];
for(const name of [before,after]){
  const report=JSON.parse(await readFile(resolve(root,'benchmarks/succinct',name,'report.json'),'utf8'));
  const dir=resolve(root,'target/succinct/runs',name,'walz'),bytes=await readFile(join(dir,'reader.wasm'));
  assert.equal(createHash('sha256').update(bytes).digest('hex'),report.reader);
  const {instantiate}=await import(pathToFileURL(join(dir,'serial/index.js')));
  runs.push({name,report,dir,module:await WebAssembly.compile(bytes),api:(await instantiate()).api});
}
const result=[];let sink=0;
async function time(f,n){const start=performance.now();for(let i=0;i<n;i++){const a=f();sink^=a[0]??0;sink^=a.at(-1)??0;}return(performance.now()-start)/n;}
for(let row=0;row<runs[0].report.rows.length;row++){
  const a=runs[0].report.rows[row],b=runs[1].report.rows[row];assert.equal(a.dataHash,b.dataHash);assert.equal(a.queryHash,b.queryHash);
  const saved=resolve(root,'target/succinct/runs',before),input=new Uint8Array(await readFile(join(saved,`${row}-data.bin`)));
  const qb=await readFile(join(saved,`${row}-queries.bin`)),q=new Int32Array(qb.buffer.slice(qb.byteOffset,qb.byteOffset+qb.byteLength));
  const variants=[];let indexBytes;
  for(const run of runs){
    const index=['rank','select'].includes(a.kind)?run.api.buildBits(input,a.n):run.api.buildWavelet(input);
    if(indexBytes)assert.deepEqual(index,indexBytes);else indexBytes=index;
    const memory=new WebAssembly.Memory({initial:256,maximum:256,shared:true});
    new Uint8Array(memory.buffer,4096,index.length).set(index);Atomics.store(new Int32Array(memory.buffer),1,1);
    const api=new WebAssembly.Instance(run.module,{env:{memory}}).exports;api.init(index.length);
    const f=()=>{const offset=api.reserve(q.length/4);new Int32Array(api.output.buffer,0,q.length).set(q);api.batch(q.length/4);return new Int32Array(api.output.buffer,offset,q.length/4).slice();};
    for(let i=0;i<5;i++)f();
    variants.push({name:run.name,f,samples:[],iterations:Math.max(1,Math.min(100,Math.ceil(20/await time(f,1))))});
  }
  assert.deepEqual(variants[0].f(),variants[1].f());
  for(let sample=0;sample<9;sample++)for(let j=0;j<2;j++){
    const v=variants[(sample+j)%2];globalThis.gc?.();v.samples.push(await time(v.f,v.iterations));
  }
  const medians=variants.map(v=>[...v.samples].sort((x,y)=>x-y)[4]);
  result.push({kind:a.kind,n:a.n,count:a.count,dataHash:a.dataHash,queryHash:a.queryHash,beforeMs:medians[0],afterMs:medians[1],ratio:medians[0]/medians[1],samples:variants.map(({name,samples,iterations})=>({name,samples,iterations}))});
}
await writeFile(join(output,'report.json'),JSON.stringify({before,after,schema:'walz-succinct-comparison-v1',node:process.version,v8:process.versions.v8,sink,result},null,2)+'\n');
const lines=['# Alternating comparison','','Same inputs and serialized indexes, captured Wasm, local execution with identical input/output copies. Nine rotated samples; milliseconds per batch.','','| Workload | Q | Before | After | Before / after |','|---|---:|---:|---:|---:|'];
for(const r of result)lines.push(`| ${r.kind} | ${r.count} | ${r.beforeMs.toFixed(3)} | ${r.afterMs.toFixed(3)} | ${r.ratio.toFixed(3)} |`);
await writeFile(join(output,'report.md'),lines.join('\n')+'\n');console.log(lines.join('\n'));
