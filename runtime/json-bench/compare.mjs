// Compare immutable binaries in one process, rotating before/after each round.
import assert from "node:assert/strict";
import {readFile,writeFile,mkdir} from "node:fs/promises";
import {join,resolve} from "node:path";
import {pathToFileURL} from "node:url";
import {cpus} from "node:os";
import {verify,measure,equivalentOutput} from "./runner.mjs";
import {hash,checksum} from "./workloads.mjs";

export function checkCompatibility(before,after) {
  for(const name of ["corpus.json","source/examples/json-parser/resident.walz","source/examples/json-parser/benchmark.walz"]) {
    assert.ok(before.hashes[name]&&after.hashes[name],`missing comparison input: ${name}`);
    assert.equal(before.hashes[name],after.hashes[name],`comparison mismatch: ${name}`);
  }
  for(const name of ["node","v8"])
    assert.equal(before.reference[name],after.reference[name],`comparison mismatch: ${name}`);
}
function input(api,source) {
  const bytes=new TextEncoder().encode(source),data=api["input-new"](bytes.length);
  for(let i=0;i<bytes.length;i++)api["input-set"](data,i,bytes[i]);
  return api["input-text"](data);
}
async function load(directory) {
  const {instance}=await WebAssembly.instantiate(await readFile(join(directory,"resident.wasm")));
  const adapters={};
  for(const backend of ["core","component"]) {
    const {instantiate}=await import(pathToFileURL(join(directory,backend,"index.js")));
    let arena;
    const {api}=await instantiate({instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);
      if(instance.exports.live)arena=instance.exports;
      return instance;
    }});
    assert.ok(arena,`${backend}: missing ABI allocator`);
    adapters[backend]={api,clean:()=>assert.equal(arena.live(),0,`${backend}: ABI buffer leak`)};
  }
  return {raw:instance.exports,adapters};
}
const metrics=["residentParse","coreParseStatus","componentParseStatus","coreNormalize","componentNormalize","coreInputLength","componentInputLength"];
export async function compare(beforeDirectory,afterDirectory,output,{rounds=9,targetMs=30}={}) {
  if(typeof global.gc!=="function")throw new Error("run with node --expose-gc");
  const manifests=await Promise.all([verify(beforeDirectory),verify(afterDirectory)]);
  checkCompatibility(...manifests);
  assert.equal(manifests[0].reference.node,process.version,"comparison Node version changed");
  assert.equal(manifests[0].reference.v8,process.versions.v8,"comparison V8 version changed");
  await mkdir(output);
  const engines={before:await load(beforeDirectory),after:await load(afterDirectory)};
  const cases=JSON.parse(await readFile(join(beforeDirectory,"corpus.json"),"utf8"));
  const report={schema:"walz-json-comparison-v1",created:new Date().toISOString(),node:process.version,
    v8:process.versions.v8,cpu:cpus()[0]?.model,arch:process.arch,rounds,targetMs,
    snapshots:{},method:{rotatingOrder:true,setupExcluded:true,explicitGc:"before each measured batch only",astCheckedOutsideTimer:true},cases:[]};
  for(const [index,name] of ["before","after"].entries()) {
    report.snapshots[name]={directory:index?afterDirectory:beforeDirectory,
      captureHash:hash(await readFile(join(index?afterDirectory:beforeDirectory,"capture.json"))),sizes:manifests[index].sizes};
  }
  for(const c of cases) {
    const expected=equivalentOutput(engines.before.adapters.core.api.normalize(c.source),c.normalized);
    const expectedHash=checksum(expected),variants={};
    for(const [name,{raw,adapters}] of Object.entries(engines)) {
      const source=input(raw,c.source);
      variants[name+".residentParse"]={fn:()=>raw.parse(source),check:ast=>assert.equal(raw.checksum(ast),expectedHash)};
      for(const [backend,{api,clean}] of Object.entries(adapters)) {
        assert.deepEqual(api.normalize(c.source),{tag:"ok",val:expected},`${name} ${backend}: output changed`);
        variants[name+"."+backend+"ParseStatus"]={fn:()=>api.parseStatus(c.source),check:x=>assert.equal(x,1),clean};
        variants[name+"."+backend+"Normalize"]={fn:()=>api.normalize(c.source),check:x=>assert.deepEqual(x,{tag:"ok",val:expected}),clean};
        variants[name+"."+backend+"InputLength"]={fn:()=>api.inputLength(c.source),check:x=>assert.equal(x,c.bytes),clean};
      }
    }
    const results=await measure(variants,{rounds,targetMs});
    const speedup=Object.fromEntries(metrics.map(metric=>[metric,results["before."+metric].medianNs/results["after."+metric].medianNs]));
    report.cases.push({name:c.name,bytes:c.bytes,inputHash:c.inputHash,outputHash:hash(expected),results,speedup});
    console.log(`${c.name}: resident ${(results["before.residentParse"].medianNs/1000).toFixed(2)} → ${(results["after.residentParse"].medianNs/1000).toFixed(2)}us (${speedup.residentParse.toFixed(2)}x)`);
  }
  // Also freeze the comparison implementation, not only its two input captures.
  for(const name of ["compare.mjs","runner.mjs","workloads.mjs"]) {
    const bytes=await readFile(new URL(name,import.meta.url));
    await writeFile(join(output,name),bytes,{flag:"wx"});
    (report.harnessHashes??={})[name]=hash(bytes);
  }
  await writeFile(join(output,"report.json"),JSON.stringify(report,null,2)+"\n",{flag:"wx"});
  const tables=metrics.map(metric=>`## ${metric}\n\n| input | before µs | after µs | speedup |\n|---|---:|---:|---:|\n`+
    report.cases.map(c=>`| ${c.name} | ${(c.results["before."+metric].medianNs/1000).toFixed(2)} | ${(c.results["after."+metric].medianNs/1000).toFixed(2)} | ${c.speedup[metric].toFixed(2)}x |`).join("\n")).join("\n\n");
  await writeFile(join(output,"report.md"),`# JSON parser tuning comparison\n\n${process.version} / ${report.cpu}; ${rounds} rotating rounds; medians. Both binaries run in the same process. Input and benchmark-program hashes match. All raw AST hashes and exact normalized outputs match the baseline. Setup and validation are outside timing; automatic GC inside a batch is included. ABI live allocations are zero after each batch.\n\n${tables}\n`,{flag:"wx"});
  return report;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  const [before,after,output,rounds]=process.argv.slice(2);
  assert.ok(before&&after&&output,"usage: compare.mjs before-capture after-capture new-output [rounds]");
  await compare(resolve(before),resolve(after),resolve(output),{rounds:rounds===undefined?9:Number(rounds)});
}
