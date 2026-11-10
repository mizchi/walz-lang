import assert from "node:assert/strict";
import {readFile,writeFile,mkdir} from "node:fs/promises";
import {join} from "node:path";
import {pathToFileURL} from "node:url";
import {performance} from "node:perf_hooks";
import {cpus} from "node:os";
import {execFile as execFileCallback} from "node:child_process";
import {promisify} from "node:util";
import {hash,checksum} from "./workloads.mjs";
const execFile=promisify(execFileCallback);
let sink;
const median=xs=>[...xs].sort((a,b)=>a-b)[Math.floor(xs.length/2)];
export async function verify(directory) {
  const manifest=JSON.parse(await readFile(join(directory,"capture.json"),"utf8"));
  for(const [name,expected] of Object.entries(manifest.hashes))
    assert.equal(hash(await readFile(join(directory,name))),expected,`snapshot changed: ${name}`);
  return manifest;
}
function batch(fn,calls) {
  const start=performance.now();
  for(let i=0;i<calls;i++)sink=fn();
  return (performance.now()-start)*1e6;
}
export async function measure(variants,{rounds=9,targetMs=30}={}) {
  assert.ok(Number.isInteger(rounds)&&rounds>=5&&rounds<=31);
  assert.ok(targetMs>=1&&targetMs<=100);
  const entries=Object.entries(variants),results={};
  const sample=async(v,n)=>{
    const elapsed=v.batch?await v.batch(n):batch(v.fn,n);
    if(!v.batch)v.check(sink);
    v.clean?.();
    return elapsed;
  };
  for(const [name,v] of entries) {
    if(v.fn)v.check(v.fn());
    let calls=1;
    while(calls<1<<20 && await sample(v,calls)<targetMs*1e6)calls*=2;
    for(let i=0;i<3;i++)await sample(v,calls);
    results[name]={callsPerSample:calls,samplesNs:[]};
  }
  for(let round=0;round<rounds;round++)for(let i=0;i<entries.length;i++) {
    const [name,v]=entries[(i+round)%entries.length],r=results[name];
    global.gc?.(); // Explicit collection is outside the measured region.
    r.samplesNs.push(await sample(v,r.callsPerSample)/r.callsPerSample);
  }
  for(const r of Object.values(results)) {
    r.medianNs=median(r.samplesNs);r.minNs=Math.min(...r.samplesNs);r.maxNs=Math.max(...r.samplesNs);
  }
  return results;
}
export function equivalentOutput(result,expected) {
  assert.equal(result.tag,"ok");
  assert.deepEqual(JSON.parse(result.val),JSON.parse(expected),"normalized JSON value differs");
  return result.val;
}
function residentSource(api,source) {
  const bytes=new TextEncoder().encode(source),data=api['input-new'](bytes.length);
  for(let i=0;i<bytes.length;i++)api['input-set'](data,i,bytes[i]);
  return api['input-text'](data);
}
async function native(directory,mode,c) {
  const file=join(directory,"inputs",c.name+".json"),binary=join(directory,"native-json-bench");
  const {stdout}=await execFile(binary,["check",file,"1"]);const check=JSON.parse(stdout);
  assert.deepEqual(JSON.parse(check.normalized),JSON.parse(c.normalized),`serde preflight: ${c.name}`);
  assert.equal(check.kind,c.kind);
  return {batch:async calls=>{
    const {stdout}=await execFile(binary,[mode,file,String(calls)]);const result=JSON.parse(stdout);
    assert.equal(result.calls,calls);
    assert.equal(result.checksum,calls*(mode==="parse"?c.kind:Buffer.byteLength(check.normalized)));
    return result.elapsedNs;
  },outputBytes:Buffer.byteLength(check.normalized)};
}
const groups={
  parse:["nodeParse","serdeParse","walzResident"],
  api:["coreInputLength","coreParseStatus","componentInputLength","componentParseStatus"],
  normalize:["nodeNormalize","serdeNormalize","coreNormalize","componentNormalize"],
};
function tables(report) {
  const titles={parse:"既存の入力からASTを生成（ABIコピーなし）",api:"JS API：入力変換を含む解析と入力長だけの対照",normalize:"解析＋再シリアライズ（WALZは入出力ABI込み）"};
  return Object.entries(groups).map(([group,names])=>[
    `## ${titles[group]}`,"",
    `| input | bytes | ${names.join(" | ")} |`,
    `|---|---:|${names.map(()=>"---:").join("|")}|`,
    ...report.cases.map(c=>`| ${c.name} | ${c.bytes} | ${names.map(name=>(c.results[name].medianNs/1000).toFixed(2)).join(" | ")} |`),"",
  ].join("\n")).join("\n");
}
export async function run(directory,output,{rounds=9,targetMs=30}={}) {
  if(typeof global.gc!=="function")throw new Error("run with node --expose-gc");
  const manifest=await verify(directory);
  await mkdir(output); // Reports, like input captures, are never overwritten.
  const corpus=JSON.parse(await readFile(join(directory,"corpus.json"),"utf8"));
  const {instance}=await WebAssembly.instantiate(await readFile(join(directory,"resident.wasm")));
  const raw=instance.exports,adapters={};
  for(const backend of ["core","component"]) {
    const {instantiate}=await import(pathToFileURL(join(directory,backend,"index.js")));
    let arena;
    const program=await instantiate({instantiateCore:async(m,i)=>{
      const instance=await WebAssembly.instantiate(m,i);if(instance.exports.live)arena=instance.exports;return instance;
    }});
    adapters[backend]={api:program.api,clean:()=>assert.equal(arena.live(),0,`${backend}: ABI buffer leak`)};
  }
  const report={schema:"walz-json-bench-v1",created:new Date().toISOString(),node:process.version,v8:process.versions.v8,
    cpu:cpus()[0]?.model,platform:process.platform,arch:process.arch,rounds,targetMs,
    captureHash:hash(await readFile(join(directory,"capture.json"))),reference:manifest.reference,sizes:manifest.sizes,
    method:{setupExcluded:true,ioExcluded:true,rotatingOrder:true,explicitGc:"before each measured batch only",nativeDropIncluded:true},cases:[]};
  for(const c of corpus) {
    const input=residentSource(raw,c.source);
    const walzOutput=equivalentOutput(adapters.core.api.normalize(c.source),c.normalized);
    assert.equal(equivalentOutput(adapters.component.api.normalize(c.source),c.normalized),walzOutput);
    const walzHash=checksum(walzOutput);
    const parseCheck=value=>assert.equal(raw.checksum(value),walzHash,`${c.name}: raw AST mismatch`);
    const variants={
      nodeParse:{fn:()=>JSON.parse(c.source),check:value=>assert.deepEqual(value,JSON.parse(c.normalized))},
      serdeParse:await native(directory,"parse",c),
      walzResident:{fn:()=>raw.parse(input),check:parseCheck},
      nodeNormalize:{fn:()=>JSON.stringify(JSON.parse(c.source)),check:value=>assert.equal(value,c.normalized)},
      serdeNormalize:await native(directory,"normalize",c),
    };
    for(const [backend,{api,clean}] of Object.entries(adapters)) {
      variants[backend+"InputLength"]={fn:()=>api.inputLength(c.source),check:value=>assert.equal(value,c.bytes),clean};
      variants[backend+"ParseStatus"]={fn:()=>api.parseStatus(c.source),check:value=>assert.equal(value,1),clean};
      variants[backend+"Normalize"]={fn:()=>api.normalize(c.source),check:value=>assert.deepEqual(value,{tag:"ok",val:walzOutput}),clean};
    }
    const results=await measure(variants,{rounds,targetMs});
    for(const result of Object.values(results))result.mibPerSecond=c.bytes/(result.medianNs/1e9)/(1024*1024);
    report.cases.push({name:c.name,bytes:c.bytes,inputHash:c.inputHash,normalizedHash:c.normalizedHash,walzNormalizedHash:walzHash,outputBytes:{walz:Buffer.byteLength(walzOutput),node:Buffer.byteLength(c.normalized),serde:variants.serdeNormalize.outputBytes},results});
    console.log(`${c.name}: parse Node ${(results.nodeParse.medianNs/1000).toFixed(2)}us, serde ${(results.serdeParse.medianNs/1000).toFixed(2)}us, Walz ${(results.walzResident.medianNs/1000).toFixed(2)}us`);
  }
  // Same API, intentionally different number representations. Not ranked inputs.
  report.semanticProbes=[];
  for(const source of ["9007199254740993","1.2300e+45"])
    report.semanticProbes.push({source,node:JSON.stringify(JSON.parse(source)),core:adapters.core.api.normalize(source),component:adapters.component.api.normalize(source)});
  await writeFile(join(output,"report.json"),JSON.stringify(report,null,2)+"\n",{flag:"wx"});
  const text=`# JSON parser speed comparison\n\n${report.node} / ${report.cpu} / ${report.arch}; ${rounds} rounds; median µs/call.\n\n`+
    `All logical JSON outputs and raw AST serialization hashes passed. AST/output validation, file I/O, compilation, instantiation and input setup are excluded. Calls rotate order each round. Explicit GC runs before each timed batch; automatic GC during a batch is included. Rust release Value destruction is inside its timer; each native process warms its allocator before timing.\n\n`+
    `Walz resident returns an opaque GC AST and is checked by a serialization hash outside the timer. Core/Component parse-status fully parses but returns only success, so it is a different API from returning an AST. Input-length measures the same input ABI without parsing; timings are not subtracted. Normalize includes both Walz ABI copies. Native Rust has no JS FFI or UTF-16 conversion. Walz preserves numeric lexemes and duplicates, JS uses Numbers, serde_json uses its default Value/Map. Ranked inputs use finite safe numbers and unique, sorted keys. Unicode escaping can differ (Walz uses \\u000a for newline, Node uses \\n); output sizes are recorded. Results apply to these inputs and representations.\n\n`+tables(report);
  await writeFile(join(output,"report.md"),text,{flag:"wx"});
  console.log(output);return report;
}
