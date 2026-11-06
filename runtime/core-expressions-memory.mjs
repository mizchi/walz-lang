// Finite, repeatable leak regression for the five typed-core extensions.
// Build in child processes; only runtime code participates in the heap samples.
import assert from "node:assert/strict";
import {execFile as execFileCallback} from "node:child_process";
import {promisify} from "node:util";
import {mkdir,readFile,readdir,writeFile} from "node:fs/promises";
import {resolve,join} from "node:path";
import {fileURLToPath,pathToFileURL} from "node:url";
import {createHash} from "node:crypto";
import {collect,tick,instrument,instrumentScopes} from "./testing/memory.mjs";

const execFile=promisify(execFileCallback);
const root=fileURLToPath(new URL("..",import.meta.url));
const directory=resolve(process.argv[2]??"target/core-expressions-memory/current");
if(typeof global.gc!=="function")throw new Error("use node --expose-gc");
await mkdir(directory,{recursive:true});
const run=(file,args)=>execFile(file,args,{cwd:root,maxBuffer:8*1024*1024});
const hash=bytes=>createHash("sha256").update(bytes).digest("hex");
const report={version:1,date:new Date().toISOString(),node:process.version,platform:process.platform,arch:process.arch,
  head:(await run("git",["rev-parse","HEAD"])).stdout.trim(),
  method:{warmupBatches:1,measuredBatches:6,gcsPerSample:4,maxCollectionPasses:8,retainedGrowthLimit:2*1024*1024},cases:[]};
const artifacts={},inspectionArtifacts={};
async function fingerprint(dir) {
  const files={};
  for(const name of (await readdir(dir)).sort())if(/\.(?:wasm|m?js|json)$/.test(name))files[name]=hash(await readFile(join(dir,name)));
  return files;
}
async function build(backend,source,name,policy) {
  const out=join(directory,backend,name);
  // The builders' CLI accepts source/output only; binding builds use their API
  // in an isolated child so jco never enters the measured process.
  const builder=pathToFileURL(join(root,"runtime",backend==="core"?"build-core.mjs":"build.mjs")).href;
  await run(process.execPath,["--input-type=module","-e",
    `const {build}=await import(${JSON.stringify(builder)});await build(${JSON.stringify(source)},${JSON.stringify(out)},${JSON.stringify(policy?{bindings:policy}:{})});`]);
  artifacts[`${backend}/${name}`]=await fingerprint(out);
  await instrument(out,backend);await instrumentScopes(out);
  inspectionArtifacts[`${backend}/${name}`]=await fingerprint(out);
  return import(pathToFileURL(join(out,"index.js")));
}
function capture(options={}) {
  const state={};
  return {state,options:{...options,instantiateCore:async(module,imports)=>{
    const instance=await WebAssembly.instantiate(module,imports);
    if(instance.exports.alloc)state.allocator=instance.exports;
    return instance;
  }}};
}
function clean(program,allocator,label) {
  assert.equal(allocator.live(),0,`${label}: linear allocation left live`);
  const tables=program.__memory(),scopes=program.__taskMemory();
  assert.ok(Object.values(tables).every(n=>n===0),`${label}: ${JSON.stringify(tables)}`);
  assert.ok(Object.values(scopes).every(n=>n===0),`${label}: ${JSON.stringify(scopes)}`);
  return {live:allocator.live(),linearBytes:allocator.memory.buffer.byteLength,tables,scopes};
}
async function sample(backend,name,work,program,allocator,extras=()=>({})) {
  await work();await collect();
  const samples=[];
  for(let round=0;round<report.method.measuredBatches;round++) {
    await work();const memory=await collect();
    const observation={...memory,...clean(program,allocator,name),...extras()};samples.push(observation);
    process.stdout.write(JSON.stringify({backend,name,round,...observation})+"\n");
  }
  const first=samples[0],last=samples.at(-1);
  const heapGrowth=last.heapUsed-first.heapUsed,arrayBufferGrowth=last.arrayBuffers-first.arrayBuffers;
  assert.ok(heapGrowth<report.method.retainedGrowthLimit,`${backend}/${name}: heap grew ${heapGrowth}`);
  assert.ok(arrayBufferGrowth<report.method.retainedGrowthLimit,`${backend}/${name}: array buffers grew ${arrayBufferGrowth}`);
  assert.equal(last.linearBytes,first.linearBytes,`${name}: arena did not stabilize after warmup`);
  report.cases.push({backend,name,heapGrowth,arrayBufferGrowth,samples});
}
function sameBytes(actual,input) {
  assert.equal(actual.length,input.length);assert.equal(actual[0],input[0]);assert.equal(actual.at(-1),input.at(-1));
}
function sameIds(actual,input) {
  assert.equal(actual.length,input.length);assert.equal(actual[0],input[0]);assert.equal(actual.at(-1),input.at(-1));
}
class Files {
  constructor() {this.opened=0;this.closed=0;this.live=new Set();this.refs=[];this.track=true;this.broken=false;}
  open() {
    const value={id:++this.opened,data:new Uint8Array(16384)};value.data.fill(value.id&255);
    this.live.add(value.id);
    if(this.track)this.refs.push(new WeakRef(value));
    return value;
  }
  read(value) {assert.ok(this.live.has(value.id),"resource used after close");return value.id;}
  close(value) {
    assert.ok(this.live.delete(value.id),"resource closed twice");this.closed++;
    if(this.broken)throw new Error("destructor failed");
  }
  imports() {return {files:{open:this.open.bind(this),read:this.read.bind(this),close:this.close.bind(this)}};}
  collected() {
    assert.equal(this.closed,this.opened);assert.equal(this.live.size,0);
    const count=this.refs.length;
    assert.ok(this.refs.every(r=>r.deref()===undefined),"closed resources retained while instance stays live");
    this.refs=[];return count;
  }
}
// Keep this producer's closure independent of the instantiateCore callback.
function sleeper() {
  const next=Promise.withResolvers();
  return {next,pending:null,sleep() {
    const pending=Promise.withResolvers();this.pending=pending;
    pending.promise[Symbol.asyncDispose]=async()=>{pending.resolve();await tick();};
    next.resolve();return pending.promise;
  }};
}
async function discardedHistory(runtime,mode) {
  const files=new Files(),sleep=sleeper();files.broken=mode==="destructor-failure";
  const captured=capture({imports:files.imports(),eventLoop:{sleep:sleep.sleep.bind(sleep)}});
  const instance=await runtime.instantiate(captured.options);
  let task,error,pending;
  if(mode==="cancel") {
    task=instance.start("pending");await sleep.next.promise;
    assert.equal(instance.__taskMemory().resources,1,"positive control: pending using has an owner");
    pending=sleep.pending.promise;await task.cancel("audit");
  } else task=instance.start(mode==="trap"?"trap":mode==="destructor-failure"?"earlyReturn":mode==="branch"?"earlyBranch":mode==="conversion-failure"?"ids":"earlyReturn",mode==="conversion-failure"?[[42n,"invalid"]]:[]);
  try {await task.result;}catch(e){error=e;}
  if(["cancel","trap","destructor-failure","conversion-failure"].includes(mode))assert.ok(error,`${mode}: expected failure`);
  await tick();clean(instance,captured.state.allocator,mode);
  assert.equal(files.opened,files.closed);
  return {mode,task,error,pending,ref:new WeakRef(captured.state.allocator.memory),resourceRefs:files.refs};
}

async function verifyCollected(refs,label,maxPasses=8) {
  // A successful deref pins the target until the current job ends. Never keep
  // dereferenced targets, and always cross a job boundary before the next GC.
  for(let pass=1;pass<=maxPasses;pass++) {
    await collect();
    if(refs.every(ref=>ref.deref()===undefined))return pass;
  }
  throw new Error(`${label}: references remain after ${maxPasses*4} full GC requests`);
}

async function rawGc() {
  const path=join(directory,"raw.wasm");
  await run("cargo",["run","--locked","--quiet","--","build","tests/gc-fixtures/core-expressions.wlt",path]);
  const bytes=await readFile(path);artifacts.raw=hash(bytes);
  const {instance}=await WebAssembly.instantiate(bytes);const api=instance.exports;
  const root=api.root();
  const pinned=(()=>{const box=api.make();api.pin(root,box);return new WeakRef(box);})();
  // This is deliberately reachable only through a Wasm field. The same
  // observer must reject it, then accept it after that real root is cleared.
  await assert.rejects(verifyCollected([pinned],"live Wasm root",2),/references remain/);
  assert.notEqual(pinned.deref(),undefined,"positive control lost its Wasm GC root");
  api.clear(root);await verifyCollected([pinned],"cleared Wasm root");
  const refs=[];
  const make=mode=>{const box=api.make();const data=api.data(box);assert.equal(api.exercise(box,mode),42);return [new WeakRef(box),new WeakRef(data)];};
  for(let mode=0;mode<7;mode++)for(let i=0;i<32;i++)refs.push(...make(mode));
  const collectionPasses=await verifyCollected(refs,"completed guest frames");
  report.rawGc={cases:["return","br","br_if","result.do","owned normal","owned catch","owned closure"],
    checkedObjects:refs.length,collectionPasses,allCollected:true,pinnedControl:true,clearedRootCollected:true};
}
await rawGc();
for(const backend of ["core","component"]) {
  const runtime=await build(backend,"tests/component-fixtures/core-expressions-memory.wlt","values");
  const {state,options}=capture();const program=await runtime.instantiate(options);const {api}=program;
  // Positive control proves live() observes an intentionally held ABI buffer.
  const pointer=state.allocator.alloc(64);assert.equal(state.allocator.live(),1);state.allocator.free(pointer);
  await sample(backend,"result-return-branch-owned-exception",async()=>{
    for(let i=0;i<400;i++) {
      const input=new Uint8Array(1024+(i%32)*1024);input.fill(i&255);
      const result=api.chain(input,i%3);
      if(i%3===0){assert.equal(result.tag,"ok");sameBytes(result.val,input);}else{assert.deepEqual(result,{tag:"err",val:1});}
      sameBytes(api.early(input),input);sameBytes(api.branch(input,i%2),input);sameBytes(api.owned(input,i%2),input);
      assert.equal(api.sqrt(1764),42);
    }
  },program,state.allocator);
  await sample(backend,"newtype-list-future-stream",async()=>{
    for(let i=0;i<200;i++) {
      const input=BigInt64Array.from({length:16+(i%32)*16},(_,n)=>BigInt(n-i));
      sameIds(api.ids(input),input);sameIds(api.ints(input),input);
      sameIds(await api.ready(input),input);sameIds(await api.await(Promise.resolve(input)),input);
      const values=await Array.fromAsync(api.stream(input));assert.equal(values.length,1);sameIds(values[0],input);
      sameIds(await api.first([input]),input);
    }
  },program,state.allocator);
  const resources=await build(backend,"examples/resources/main.wlt","resources","examples/resources/policy.json");
  const files=new Files(),captured=capture({imports:files.imports()});
  const managed=await resources.instantiate(captured.options);
  let collectedResources=0;
  await sample(backend,"using-normal-err-return-branch-recur-catch",async()=>{
    for(let i=0;i<100;i++) {
      files.track=i<4;
      assert.ok(managed.api.normal()>0);assert.deepEqual(managed.api.err(),{tag:"err",val:7});
      managed.api.explicit();managed.api.nested();assert.equal(managed.api.caught(),42);
      assert.ok(managed.api.earlyReturn()>0);assert.ok(managed.api.earlyBranch()>0);
      assert.equal(managed.api.conditional(1),42);assert.ok(managed.api.conditional(0)>0);
      assert.ok(managed.api.innerBranch()>0);assert.ok(managed.api.recur()>0);
    }
  },managed,captured.state.allocator,()=>{
    collectedResources+=files.collected();return {opened:files.opened,closed:files.closed,collectedResources};
  });
  // Keep finished handles, errors and producer Promises while dropping each store.
  const histories=[];
  for(const mode of ["return","branch","cancel","trap","destructor-failure","conversion-failure"]) {
    histories.push(await discardedHistory(mode==="conversion-failure"?runtime:resources,mode));
  }
  const collectionPasses=await verifyCollected(histories.flatMap(h=>[h.ref,...h.resourceRefs]),`${backend}: completed histories`);
  report.cases.push({backend,name:"discarded-instances-with-held-history",modes:histories.map(h=>h.mode),collectionPasses,allCollected:true});
}
report.artifacts=artifacts;report.inspectionArtifacts=inspectionArtifacts;
report.sources={};
const sourceFiles=(await run("git",["ls-files","-co","--exclude-standard","src","stdlib","runtime"])).stdout.trim().split("\n");
for(const file of [...new Set([...sourceFiles,"tests/gc-fixtures/core-expressions.wlt","tests/component-fixtures/core-expressions-memory.wlt","examples/resources/main.wlt","examples/resources/policy.json","Cargo.lock","pnpm-lock.yaml"])].sort())
  report.sources[file]=hash(await readFile(join(root,file)));
await writeFile(join(directory,"report.json"),JSON.stringify(report,null,2)+"\n");
console.log(`memory checks passed: ${join(directory,"report.json")}`);
