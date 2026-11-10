// Immutable captures separate build-time tools from the measured process.
import {mkdir,readFile,writeFile,readdir,copyFile,chmod} from "node:fs/promises";
import {join,resolve,dirname,relative} from "node:path";
import {fileURLToPath,pathToFileURL} from "node:url";
import {execFile as execFileCallback} from "node:child_process";
import {promisify} from "node:util";
import {cpus} from "node:os";
import {corpus,hash} from "./json-bench/workloads.mjs";
import {verify} from "./json-bench/runner.mjs";
const execFile=promisify(execFileCallback),root=fileURLToPath(new URL("..",import.meta.url));
async function files(directory) {
  const out=[];
  for(const entry of await readdir(directory,{withFileTypes:true})) {
    const path=join(directory,entry.name);
    out.push(...entry.isDirectory()?await files(path):[path]);
  }
  return out.sort();
}
async function copy(source,target) {await mkdir(dirname(target),{recursive:true});await copyFile(source,target);}
async function capture(directory) {
  await mkdir(dirname(directory),{recursive:true});await mkdir(directory);
  for(const folder of ["src","walz/core","examples/json-parser"])
    for(const file of await files(join(root,folder)))await copy(file,join(directory,"source",relative(root,file)));
  for(const name of ["Cargo.toml","Cargo.lock","pnpm-lock.yaml","tools/json-bench.rs"])
    await copy(join(root,name),join(directory,"source",name));
  await copy(fileURLToPath(import.meta.url),join(directory,"capture-tool.mjs"));
  for(const name of ["runner.mjs","workloads.mjs"])await copy(join(root,"runtime/json-bench",name),join(directory,name));
  const cases=corpus();await mkdir(join(directory,"inputs"));
  for(const c of cases)await writeFile(join(directory,"inputs",c.name+".json"),c.source,{flag:"wx"});
  await writeFile(join(directory,"corpus.json"),JSON.stringify(cases,null,2)+"\n",{flag:"wx"});
  for(const backend of ["core","component"]) {
    const {build}=await import(backend==="core"?"./build-core.mjs":"./build.mjs");
    await build(join(root,"examples/json-parser/benchmark.walz"),join(directory,backend));
  }
  const command=(args)=>execFile("cargo",args,{cwd:root,maxBuffer:4*1024*1024});
  await command(["run","--locked","--quiet","--","build","examples/json-parser/resident.walz",join(directory,"resident.wasm")]);
  await command(["build","--locked","--release","--example","json-bench"]);
  await copy(join(root,"target/release/examples/json-bench"),join(directory,"native-json-bench"));await chmod(join(directory,"native-json-bench"),0o755);
  const lock=await readFile(join(root,"Cargo.lock"),"utf8");
  const rustc=(await execFile("rustc",["--version"])).stdout.trim();
  const manifest={schema:"walz-json-capture-v1",created:new Date().toISOString(),node:process.version,cpu:cpus()[0]?.model,
    compiler:{head:(await execFile("git",["rev-parse","HEAD"],{cwd:root})).stdout.trim(),dirty:!!(await execFile("git",["status","--porcelain"],{cwd:root})).stdout.trim()},
    reference:{node:process.version,v8:process.versions.v8,rustc,serdeJson:lock.match(/name = "serde_json"\nversion = "([^"]+)"/)[1],profile:"release, default serde_json features"},hashes:{},sizes:{}};
  for(const file of await files(directory)) {
    const bytes=await readFile(file);manifest.hashes[relative(directory,file)]=hash(bytes);
    if(file.endsWith(".wasm")||file.endsWith("native-json-bench"))manifest.sizes[relative(directory,file)]=bytes.length;
  }
  await writeFile(join(directory,"capture.json"),JSON.stringify(manifest,null,2)+"\n",{flag:"wx"});console.log(directory);
}
const [mode,first,second,third]=process.argv.slice(2);
if(mode==="capture"&&first&&!second)await capture(resolve(first));
else if(mode==="run"&&first&&second) {
  const directory=resolve(first);await verify(directory);
  const {run}=await import(pathToFileURL(join(directory,"runner.mjs")));
  await run(directory,resolve(second),{rounds:third===undefined?9:Number(third)});
}else throw new Error("usage: json-parser-bench.mjs capture directory | run snapshot output-directory [rounds]");
