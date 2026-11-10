import assert from 'node:assert/strict';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {cp,mkdir,readFile,writeFile,readdir} from 'node:fs/promises';
import {join,resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {createRunDirectory} from './support.mjs';
import {metrics,sourceIncludes} from '../lib/source.mjs';
import {instrument} from '../../runtime/testing/memory.mjs';
import {build as buildComponent} from '../../runtime/build.mjs';
import {createEventLoop} from '../../runtime/host.mjs';

const execFile=promisify(callback),root=fileURLToPath(new URL('../../',import.meta.url));
const binary=resolve(process.env.WALZ??join(root,'target/debug/walz'));
const output=await createRunDirectory(join(root,'evals/runs'),process.argv[2]);
const snapshot=join(output,'snapshot'),work=join(output,'work');await mkdir(work,{recursive:true});
await cp(join(root,'evals/macro-usability'),join(snapshot,'suite'),{recursive:true});
for(const file of ['docs/user-macros.md','docs/lsp.md','CHEATSHEET.md']){
  await mkdir(dirname(join(snapshot,file)),{recursive:true});await cp(join(root,file),join(snapshot,file));
}
const suite=join(snapshot,'suite'),app=join(suite,'app');
const variants=['inline','functions','macros'];
const model=await readFile(join(app,'model.walz.in'),'utf8');
for(const variant of variants){
  await writeFile(join(app,`model-${variant}.walz`),model.replace('@PARSER@',`./parse-${variant}.walz`));
  await writeFile(join(app,`main-${variant}.walz`),`(module
  (@use "./model-${variant}.walz" (func $main $sample) (func $summarize $aggregate) (func $summarize-nonnegative $nonnegative))
  (export "main" (func $sample))
  (export "summarize" (func $aggregate))
  (export "summarize-nonnegative" (func $nonnegative)))\n`);
}
await writeFile(join(app,'macro.test.walz'),`(module
  (@use "./model-macros.walz" (func $main))
  (@use "walz:core" (func $bytes.equal))
  (@test "macro JSON application"
    (call $bytes.equal (@string.bytes (call $main))
      (@string.bytes (@string.lit "{\\\"accepted\\\":2,\\\"rejected\\\":1,\\\"totals\\\":{\\\"food\\\":42}}")))))\n`);
await writeFile(join(app,'macro_bench.walz'),`(module (@use "./model-macros.walz" (func $main))
  (@bench "macro JSON validation" (result i32) (@bytes.len (@string.bytes (call $main)))))\n`);
const {oracle,parseReport,cases,scalingSource}=await import(pathToFileURL(join(suite,'support.mjs')));
const {probe:probeLsp}=await import(pathToFileURL(join(suite,'lsp.mjs')));
for(const n of [1,8,32,64,128,256])for(const macro of [false,true]){
  await mkdir(join(suite,'scaling'),{recursive:true});
  await writeFile(join(suite,'scaling',`${macro?'macro':'inline'}-${n}.walz`),scalingSource(n,macro));
}
const sha=b=>createHash('sha256').update(b).digest('hex');
async function hashes(directory,prefix=''){
  const result={};for(const entry of await readdir(directory,{withFileTypes:true})){
    const path=join(directory,entry.name),key=prefix+entry.name;
    if(entry.isDirectory())Object.assign(result,await hashes(path,key+'/'));else result[key]=sha(await readFile(path));
  }return result;
}
const inputs=await hashes(snapshot);await writeFile(join(output,'inputs.json'),JSON.stringify(inputs,null,2)+'\n');
const implementation={};
for(const directory of ['src','walz/core','crates/walz-web/src','runtime'])Object.assign(implementation,await hashes(join(root,directory),directory+'/'));
for(const file of ['Cargo.toml','Cargo.lock','build.rs','package.json','pnpm-lock.yaml'])implementation[file]=sha(await readFile(join(root,file)));
const git=(await execFile('git',['rev-parse','HEAD'],{cwd:root})).stdout.trim();
await writeFile(join(output,'manifest.json'),JSON.stringify({schema:'walz-macro-usability-v1',inputRevision:4,referenceLanguage:'en',language:'walz',extensionPrefix:'@',createdAt:new Date().toISOString(),perspective:'nonblind implementer dogfooding',binary,binarySha256:sha(await readFile(binary)),git,node:process.version,platform:process.platform,architecture:process.arch,implementation},null,2)+'\n');

const checks=[],commands=[],exploratory=[],timings=[],sizes=[],sourceMetrics={},instances=new Map();
async function command(args){
  const start=performance.now();
  const result=await execFile(binary,args,{cwd:root,maxBuffer:16*1024*1024,timeout:60000}).then(r=>({code:0,...r}),e=>({code:e.code??'spawn-error',stdout:e.stdout??'',stderr:e.stderr??'',signal:e.signal??null}));
  const record={args,milliseconds:performance.now()-start,...result};commands.push(record);return record;
}
async function checked(args){const result=await command(args);assert.equal(result.code,0,result.stderr||result.stdout);return result;}
async function check(name,fn){
  const start=performance.now();
  try{const evidence=await fn();checks.push({name,passed:true,milliseconds:performance.now()-start,evidence});console.log(`PASS ${name}`);}
  catch(e){checks.push({name,passed:false,milliseconds:performance.now()-start,error:e.message});console.log(`FAIL ${name}: ${e.message}`);}
}
async function graph(path,seen=new Set()){
  path=resolve(path);if(seen.has(path))return seen;seen.add(path);
  for(const dependency of sourceIncludes(await readFile(path,'utf8')))await graph(resolve(dirname(path),dependency),seen);return seen;
}
for(const variant of variants){
  const path=join(app,`main-${variant}.walz`),files=await graph(path);
  const perFile={};for(const file of files)perFile[file.slice(app.length+1)]=metrics(await readFile(file,'utf8'));
  sourceMetrics[variant]={consumer:metrics(await readFile(join(app,`parse-${variant}.walz`),'utf8')),files:perFile,
    totalTokens:Object.values(perFile).reduce((n,v)=>n+v.tokens,0),totalLines:Object.values(perFile).reduce((n,v)=>n+v.lines,0)};
  await check(`type check application: ${variant}`,()=>checked(['check','--json',path]));
}

await check('CLI run/test/bench workflow',async()=>{
  const run=await checked(['run',join(app,'main-macros.walz'),'--target','core-js']);
  assert.deepEqual(parseReport(JSON.parse(run.stdout)),oracle(cases[1].rows.slice(0,2).concat('invalid')));
  const test=await checked(['test',join(app,'macro.test.walz')]);
  const bench=await checked(['bench',join(app,'macro_bench.walz'),'--target','core-js','--samples','3','--iterations','5','--warmup-ms','0','--json']);
  return {run,test,bench};
});
await check('inferred generic arguments are visible in expansion',async()=>{
  const result=await checked(['macro-expand',join(app,'main-macros.walz')]);const report=JSON.parse(result.stdout);
  assert.ok(report.expansions.some(e=>Object.hasOwn(e.inferred,'?Option')));return report;
});

async function instantiate(backend,source,directory,options={}){
  if(backend==='core')await checked(['build',source,'--target','core-js','-o',directory]);else await buildComponent(source,directory);
  await instrument(directory,backend);let arena;
  const {instantiate}=await import(pathToFileURL(join(directory,'index.js')));
  const runtime=await instantiate({...options,instantiateCore:async(module,imports)=>{
    const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)arena=instance.exports;return instance;
  }});
  return {...runtime,clean(){assert.ok(arena,'ABI allocator is instrumented');assert.equal(arena.live(),0);assert.ok(Object.values(runtime.__memory()).every(n=>n===0),JSON.stringify(runtime.__memory()));}};
}
for(const backend of ['core','component'])for(const variant of variants){
  await check(`oracle, changed policy and boundary cleanup: ${backend}/${variant}`,async()=>{
    const directory=join(work,backend,variant),source=join(app,`main-${variant}.walz`);
    const runtime=await instantiate(backend,source,directory);instances.set(`${backend}/${variant}`,runtime);
    const evidence=[];
    for(const nonnegative of [false,true])for(const item of cases){
      const actual=await runtime.api[nonnegative?'summarizeNonnegative':'summarize'](item.rows);
      assert.deepEqual(parseReport(actual),oracle(item.rows,nonnegative),item.name);runtime.clean();evidence.push({case:item.name,nonnegative,rows:item.rows.length,actual});
    }
    for(const entry of await readdir(directory)){if(entry.endsWith('.wasm'))sizes.push({backend,variant,file:entry,bytes:(await readFile(join(directory,entry))).length});}
    return evidence;
  });
}

for(const backend of ['core','component']){
  await check(`evaluation order, shadowing and ABI cleanup: ${backend}`,async()=>{
    const runtime=await instantiate(backend,join(suite,'probes/order.walz'),join(work,backend,'order'));
    assert.equal(await runtime.api.main(),42);runtime.clean();return {result:42};
  });
  await check(`macro suspension and queued/running cancellation: ${backend}`,async()=>{
    let ready;let waiting=new Promise(resolve=>ready=resolve);const events=[];
    const clock=createEventLoop({onEvent:event=>{events.push(event);if(event.type==='timer-start'&&event.milliseconds===60000)ready();}});
    const runtime=await instantiate(backend,join(suite,'probes/async.walz'),join(work,backend,'async'),{eventLoop:clock});
    assert.equal(await runtime.api.main(0),42n);runtime.clean();
    const job=runtime.start('main',[60000]);const rejection=assert.rejects(job.result,e=>e.name==='TaskCancelledError');
    const timer=setTimeout(()=>ready(),5000);await waiting;clearTimeout(timer);
    const queued=runtime.start('queued');const queuedRejection=assert.rejects(queued.result,e=>e.name==='TaskCancelledError');
    assert.deepEqual(await queued.cancel(),{status:'cancelled',started:false});await queuedRejection;
    assert.deepEqual(await job.cancel(),{status:'cancelled',started:true});await rejection;
    runtime.clean();assert.equal(await runtime.api.queued(),7n);runtime.clean();
    assert.ok(events.some(e=>e.type==='dispose'&&e.milliseconds===60000));return {events};
  });
}

for(const [name,pattern] of [['wrong-type',/type mismatch/],['effect',/effect/],['double-move',/already moved|ownership/]])await check(`static rejection and diagnostic location: ${name}`,async()=>{
  const source=join(suite,`probes/${name}.walz`),result=await command(['check','--json',source]);
  assert.equal(result.code,1);const diagnostic=JSON.parse(result.stdout).diagnostics[0];assert.match(diagnostic.message,pattern);
  assert.ok(diagnostic.primary);if(name==='wrong-type')assert.equal(diagnostic.primary.location.line,4);
  return diagnostic;
});
for(const name of ['delegated-binder','repeated-binders']){
  const result=await command(['check','--json',join(suite,`probes/${name}.walz`)]);
  exploratory.push({name,desired:'compile and execute the documented authoring task',achieved:result.code===0,result});
}
await check('stdio LSP required behaviors',async()=>{
  const result=await probeLsp(binary,join(work,'editor'));
  await writeFile(join(work,'lsp.json'),JSON.stringify(result,null,2)+'\n');
  exploratory.push(...result.observations.filter(o=>!o.required));
  assert.equal(result.stderr,'');assert.ok(result.observations.filter(o=>o.required).every(o=>o.achieved),JSON.stringify(result.observations));return result.observations;
});

// Balance order across rounds; a CLI measurement includes startup and loading.
for(let round=0;round<7;round++)for(let offset=0;offset<variants.length;offset++){
  const variant=variants[(round+offset)%variants.length],source=join(app,`main-${variant}.walz`);
  for(const operation of ['check','build']){
    const args=operation==='check'?['check',source]:['build',source,'--target','core','-o',join(work,`${variant}-${round}.wasm`)];
    const result=await command(args);timings.push({kind:'cli',operation,variant,round,milliseconds:result.milliseconds,code:result.code});
  }
}
for(const n of [1,8,32,64,128,256])for(const macro of [false,true]){
  const samples=[];
  for(let round=0;round<3;round++){const result=await command(['check','--json',join(suite,'scaling',`${macro?'macro':'inline'}-${n}.walz`)]);samples.push(result);}
  exploratory.push({name:`scale ${macro?'macro':'inline'} ${n}`,desired:'all call sites compile within the existing limits',achieved:samples.every(s=>s.code===0),samples});
}
const rows=cases.at(-1).rows.slice(0,128);
for(const backend of ['core','component']){
  const active=variants.filter(v=>instances.has(`${backend}/${v}`));
  for(const variant of active)for(let i=0;i<5;i++)await instances.get(`${backend}/${variant}`).api.summarize(rows);
  for(let round=0;round<9;round++)for(let offset=0;offset<active.length;offset++){
    const variant=active[(round+offset)%active.length],runtime=instances.get(`${backend}/${variant}`);
    const start=performance.now();for(let i=0;i<20;i++)await runtime.api.summarize(rows);
    const elapsed=performance.now()-start;runtime.clean();timings.push({kind:'warm-call',backend,variant,round,iterations:20,rows:rows.length,milliseconds:elapsed/20});
  }
}

await check('frozen inputs and implementation stayed unchanged',async()=>{
  assert.deepEqual(await hashes(snapshot),inputs);
  assert.equal(sha(await readFile(binary)),JSON.parse(await readFile(join(output,'manifest.json'),'utf8')).binarySha256);
  for(const [path,hash] of Object.entries(implementation))assert.equal(sha(await readFile(join(root,path))),hash,path);return {files:Object.keys(inputs).length};
});
const summary={schema:'walz-macro-usability-result-v1',checks,exploratory,sourceMetrics,sizes,timings,
  passed:checks.every(c=>c.passed),unmetTasks:exploratory.filter(o=>!o.achieved).map(o=>o.name)};
await writeFile(join(output,'commands.json'),JSON.stringify(commands,null,2)+'\n');
await writeFile(join(output,'result.json'),JSON.stringify(summary,null,2)+'\n');
console.log(JSON.stringify({output,passed:summary.passed,required:checks.length,failed:checks.filter(c=>!c.passed).map(c=>c.name),unmet:summary.unmetTasks},null,2));
if(!summary.passed)process.exitCode=1;
