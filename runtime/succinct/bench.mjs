// Every run is new. Sources, input hashes, generated Wasm and the actual jsimd
// public API implementation are captured before measuring. No historical files
// or reference kernels are patched to improve a result.
import {build} from './build.mjs';
import {instantiate} from './index.mjs';
import {IndexedJS} from './indexed-js.mjs';
import {mkdir,readFile,writeFile,cp,readdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {cpus,platform,arch} from 'node:os';
import {execFile as execCallback} from 'node:child_process';
import {promisify} from 'node:util';
import assert from 'node:assert/strict';
const execFile=promisify(execCallback);
const root=fileURLToPath(new URL('../../',import.meta.url));
const [id,reference=resolve(root,'../jsimd')]=process.argv.slice(2);
if(!/^[a-zA-Z0-9][a-zA-Z0-9_-]{0,79}$/.test(id??''))throw new Error('usage: bench.mjs new-run-id [jsimd-checkout]');
const run=resolve(root,'target/succinct/runs',id),reportDir=resolve(root,'benchmarks/succinct',id);
await mkdir(resolve(root,'target/succinct/runs'),{recursive:true});
await mkdir(run); // refuses overwrite
await mkdir(resolve(root,'benchmarks/succinct'),{recursive:true});
await mkdir(reportDir);
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
async function hashes(dir,prefix=''){
  const out={};for(const entry of await readdir(dir,{withFileTypes:true})){
    const p=join(dir,entry.name),key=prefix+entry.name;
    if(entry.isDirectory())Object.assign(out,await hashes(p,key+'/'));else out[key]=sha(await readFile(p));
  }return out;
}
await cp(resolve(root,'examples/succinct'),join(run,'walz-source'),{recursive:true});
await cp(resolve(root,'runtime/succinct'),join(run,'runner-source'),{recursive:true});
await cp(resolve(root,'runtime/workers'),join(run,'worker-host-source'),{recursive:true});
await cp(resolve(root,'target/debug/walz'),join(run,'walz-compiler'));
for(const part of ['rank-select-bit-vector','wavelet-matrix-uint8','internal'])
  await cp(join(reference,'packages/jsimd/src',part),join(run,'reference',part),{recursive:true});
await build(join(run,'walz'));
const manifest={schema:'walz-succinct-bench-v1',id,node:process.version,v8:process.versions.v8,cpu:cpus()[0]?.model,logicalCpus:cpus().length,platform:platform(),arch:arch(),
  referenceCommit:(await execFile('git',['-C',reference,'rev-parse','HEAD'])).stdout.trim(),
  source:await hashes(join(run,'walz-source')),runner:await hashes(join(run,'runner-source')),workerHost:await hashes(join(run,'worker-host-source')),generated:await hashes(join(run,'walz')),reference:await hashes(join(run,'reference')),
  compiler:sha(await readFile(resolve(root,'target/debug/walz'))),reader:sha(await readFile(join(run,'walz/reader.wasm'))),
  methods:{samples:9,warmup:3,order:'rotated',calibrationTargetMs:15,construction:'single builder, published snapshot copied once',sharedMemoryCapacity:16777216}};
await writeFile(join(reportDir,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
const loadStart=performance.now(),sdk=await instantiate(pathToFileURL(join(run,'walz')+'/'));
const walzLoadMs=performance.now()-loadStart;
const referenceStart=performance.now();
const {RankSelectBitVector}=await import(pathToFileURL(join(run,'reference/rank-select-bit-vector/mod.ts')));
const {WaveletMatrixUint8}=await import(pathToFileURL(join(run,'reference/wavelet-matrix-uint8/mod.ts')));
const referenceLoadMs=performance.now()-referenceStart;
const module=await WebAssembly.compile(await readFile(join(run,'walz/reader.wasm')));
let seed=0x5eed1234,sink=0;
const random=()=>seed=(Math.imul(seed,1664525)+1013904223)>>>0;
function queryInput(kind,n,count){
  const q=new Int32Array(count*4),a=new Uint32Array(count),b=new Uint32Array(count),c=new Uint32Array(count);
  for(let i=0;i<count;i++){
    if(kind==='rank'){a[i]=random()%(n+1);q.set([0,a[i],0,0],4*i);}
    else if(kind==='select'){a[i]=random()%Math.floor(n/3);q.set([1,a[i],0,0],4*i);}
    else {
      let l=random()%n,r=random()%n;if(l>r)[l,r]=[r,l];if(l===r)r++;
      a[i]=l;b[i]=r;
      if(kind==='quantile'){c[i]=random()%(r-l);q.set([4,l,r,c[i]],4*i);}
      else{const lo=random()%256,hi=lo+random()%(257-lo);c[i]=lo|(hi<<16);q.set([3,l,r,c[i]],4*i);}
    }
  }return{q,a,b,c};
}
function local(bytes){
  const memory=new WebAssembly.Memory({initial:256,maximum:256,shared:true});
  new Uint8Array(memory.buffer,4096,bytes.length).set(bytes);Atomics.store(new Int32Array(memory.buffer,0,2),1,1);
  const api=new WebAssembly.Instance(module,{env:{memory}}).exports;api.init(bytes.length);
  return q=>{const count=q.length/4,offset=api.reserve(count);new Int32Array(api.output.buffer,0,q.length).set(q);api.batch(count);return new Int32Array(api.output.buffer,offset,count).slice();};
}
const now=()=>performance.now();
async function timing(f,iterations){const start=now();for(let i=0;i<iterations;i++){const out=await f();sink^=out[0]??0;sink^=out[out.length-1]??0;}return(now()-start)/iterations;}
const rows=[];
for(const spec of [
  {kind:'rank',n:4096,count:1},
  {kind:'rank',n:262144,count:32768},
  {kind:'rank',n:1048576,count:32768,shape:'sparse'},
  {kind:'select',n:262144,count:4096},
  {kind:'quantile',n:262144,count:1024},
  {kind:'quantile',n:262144,count:32768},
  {kind:'quantile',n:1048576,count:32768},
  {kind:'quantile',n:262144,count:32768,shape:'low-alphabet'},
  {kind:'range',n:262144,count:32768},
]){
  const {kind,n,count}=spec,isBits=kind==='rank'||kind==='select';
  const input=isBits?Uint8Array.from({length:Math.ceil(n/8)},()=>random()>>>24):Uint8Array.from({length:n},()=>random()>>>24);
  if(spec.shape==='sparse'){input.fill(0);for(let i=0;i<n;i++)if(random()%1000===0)input[i>>>3]|=1<<(i&7);}
  if(spec.shape==='low-alphabet')for(let i=0;i<input.length;i++)input[i]&=7;
  const queries=queryInput(kind,n,count);
  const dataHash=sha(input),queryHash=sha(new Uint8Array(queries.q.buffer));
  await writeFile(join(run,`${rows.length}-data.bin`),input);await writeFile(join(run,`${rows.length}-queries.bin`),new Uint8Array(queries.q.buffer));
  let start=now();const js=isBits?IndexedJS.bits(input,n):IndexedJS.wavelet(input);const jsBuild=now()-start;
  start=now();const ref=isBits?RankSelectBitVector.fromUint32Array(n,new Uint32Array(input.buffer)):WaveletMatrixUint8.from(input);const refBuild=now()-start;
  const expected=js.queryMany(queries.q);
  const referenceQuery=()=>{
    if(kind==='rank')return ref.rank1Many(queries.a);
    if(kind==='select')return ref.select1Many(queries.a);
    if(kind==='quantile')return ref.quantileMany(queries.a,queries.b,queries.c);
    const out=new Int32Array(count);for(let i=0;i<count;i++)out[i]=ref.rangeFreq(queries.a[i],queries.b[i],queries.c[i]&65535,queries.c[i]>>>16);return out;
  };
  assert.deepEqual([...referenceQuery()],[...expected]);
  const indexes=[],prepare={};let walzBuild;
  try{
    for(const workers of [1,2,4]){
      start=now();const index=isBits?sdk.buildBits(input,n):sdk.buildWavelet(input);const built=now()-start;
      if(workers===1)walzBuild=built;
      indexes.push(index);start=now();await index.prepare({workers});prepare[workers]=now()-start;
    }
    const synchronous=local(indexes[0].snapshot());
    const runners=[{name:'js-indexed',f:()=>js.queryMany(queries.q)},{name:'jsimd-public',f:referenceQuery},
      {name:'walz-local',f:()=>synchronous(queries.q)},
      ...indexes.map((index,i)=>({name:`walz-${[1,2,4][i]}-worker`,f:()=>index.queryMany(queries.q,{workers:[1,2,4][i]})}))];
    for(const runner of runners){const first=now();const firstOutput=await runner.f();runner.firstMs=now()-first;assert.deepEqual([...firstOutput],[...expected]);for(let i=0;i<3;i++)await runner.f();runner.iterations=Math.max(1,Math.min(100,Math.ceil(15/await timing(runner.f,1))));runner.samples=[];}
    for(let sample=0;sample<9;sample++)for(let j=0;j<runners.length;j++){
      const runner=runners[(j+sample)%runners.length];globalThis.gc?.();runner.samples.push(await timing(runner.f,runner.iterations));
    }
    for(const runner of runners)assert.deepEqual([...await runner.f()],[...expected]);
    const row={...spec,dataHash,queryHash,indexBytes:indexes[0].byteLength,buildMs:{js:jsBuild,jsimd:refBuild,walz:walzBuild},prepareMs:prepare,
      results:runners.map(({name,samples,iterations,firstMs})=>({name,iterations,firstMs,samples,medianMs:[...samples].sort((a,b)=>a-b)[4],minMs:Math.min(...samples),maxMs:Math.max(...samples)}))};
    const jsMedian=row.results[0].medianMs;
    row.estimatedBreakEvenBatches=Object.fromEntries([1,2,4].map((workers,i)=>{const saved=jsMedian-row.results[i+3].medianMs;return [workers,saved>0?Math.max(1,Math.ceil((walzBuild+prepare[workers]-jsBuild)/saved)):null];}));
    rows.push(row);console.log(kind,n,count,Object.fromEntries(row.results.map(r=>[r.name,r.medianMs.toFixed(4)])));
  }finally{await Promise.all(indexes.map(index=>index.close()));ref.dispose();}
}
const report={...manifest,walzLoadMs,referenceLoadMs,sink,rows};
await writeFile(join(reportDir,'report.json'),JSON.stringify(report,null,2)+'\n');
const lines=['# Succinct index benchmark', '',`Node ${process.version}, V8 ${process.versions.v8}, ${manifest.cpu}. Nine rotated samples; milliseconds per batch.`,
  '', 'Construction/worker startup are excluded from steady-state rows and recorded separately in report.json. Walz rows include query copy, execution and output copy; worker rows also include messages and result merging. jsimd is its captured public API. Range queries use its scalar API loop. Walz quantile outputs i32; jsimd outputs u8.',
  '', '| Workload | N | Q | Indexed JS | jsimd | Walz local | Walz 1 worker | Walz 2 | Walz 4 |','|---|---:|---:|---:|---:|---:|---:|---:|---:|'];
for(const row of rows)lines.push(`| ${row.kind}${row.shape?' ('+row.shape+')':''} | ${row.n} | ${row.count} | ${row.results.map(r=>r.medianMs.toFixed(4)).join(' | ')} |`);
lines.push('', 'No comparison attributes the indexed-select algorithm improvement over linear jsimd select to Walz alone. The indexed JS baseline uses the same directory, tail mask and upper_bound strategy.', '', `Load/compile: Walz ${walzLoadMs.toFixed(2)} ms; jsimd ${referenceLoadMs.toFixed(2)} ms. These are single observations, not steady-state medians.`);
await writeFile(join(reportDir,'report.md'),lines.join('\n')+'\n');
console.log(reportDir);
