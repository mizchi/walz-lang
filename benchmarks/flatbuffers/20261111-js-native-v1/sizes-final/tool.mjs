// Frozen-input experiment: compiler/build work is separate from measurement.
import assert from 'node:assert/strict';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {readFile,writeFile,readdir,mkdir,copyFile,cp} from 'node:fs/promises';
import {resolve,join,dirname,relative} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {gzipSync,brotliCompressSync,constants} from 'node:zlib';
import {build} from './build-js-native.mjs';
import {build as core} from './build-core.mjs';
import {hash} from './flatbuffers-bench/workloads.mjs';
import {verify} from './flatbuffers-bench/runner.mjs';
import {bundle} from './flatbuffers-size.mjs';
const exec=promisify(callback),root=fileURLToPath(new URL('..',import.meta.url));
async function files(path){const out=[];for(const e of await readdir(path,{withFileTypes:true})){const p=join(path,e.name);out.push(...e.isDirectory()?await files(p):[p]);}return out.sort();}
async function copy(source,target){await mkdir(dirname(target),{recursive:true});await copyFile(source,target);}
async function capture(directory){
 await exec(process.execPath,['runtime/flatbuffers-bench.mjs','capture',directory],{cwd:root,maxBuffer:4*1024*1024});
 for(const mode of ['utf8','js-strings']){
  await build('examples/flatbuffers/main.walz',join(directory,'direct-'+mode),{mode});
  await build('tests/component-fixtures/js-native.walz',join(directory,'text-'+mode),{mode});
 }
 await core('tests/component-fixtures/js-native.walz',join(directory,'text-core'));
 for(const name of ['runtime/js-native.mjs','runtime/build-js-native.mjs','runtime/js-native-bench.mjs','runtime/js-native.test.mjs','examples/js-native-build.rs','tests/component-fixtures/js-native.walz'])await copy(join(root,name),join(directory,'source',name));
 await copy(new URL('./flatbuffers-bench/native.mjs',import.meta.url),join(directory,'native.mjs'));
 const manifest=await verify(directory);manifest.experiment='walz-flatbuffers-js-native-v1';manifest.created=new Date().toISOString();
 for(const path of await files(directory))if(path!==join(directory,'capture.json')){
  const bytes=await readFile(path),name=relative(directory,path);manifest.hashes[name]=hash(bytes);if(name.endsWith('.wasm'))manifest.sizes[name]=bytes.length;
 }
 await writeFile(join(directory,'capture.json'),JSON.stringify(manifest,null,2)+'\n');await verify(directory);console.log(directory);
}
const features=['--enable-gc','--enable-reference-types','--enable-exception-handling','--enable-bulk-memory','--enable-mutable-globals','--enable-multivalue','--enable-multimemory','--enable-simd'];
const compress=bytes=>({raw:bytes.length,gzip:gzipSync(bytes,{level:9}).length,brotli:brotliCompressSync(bytes,{params:{[constants.BROTLI_PARAM_QUALITY]:11}}).length,sha256:hash(bytes)});
async function sizes(directory,output,optimizer){
 await verify(directory);await mkdir(output);
 const version=(await exec(optimizer,['--version'])).stdout.trim();if(!/\b133\b/.test(version))throw new Error('select Binaryen 133');
 const report={schema:'walz-js-native-size-v1',created:new Date().toISOString(),captureHash:hash(await readFile(join(directory,'capture.json'))),node:process.version,vite:(await import('vite')).version,
  optimizer:{version,sha256:hash(await readFile(optimizer))},method:'Same Vite ES/oxc and gzip9/Brotli11 method as the previous codec. Native Wasm kept external and counted separately; maps and unused files excluded. O3 uses explicit features, open world, preserves traps. Bundled outputs are actually instantiated and fully checked.',variants:{}};
 const fb=await import(pathToFileURL(join(directory,'reference-package/js/flatbuffers.js'))),{createReference}=await import(pathToFileURL(join(directory,'reference.mjs'))),reference=createReference(fb);
 const {materialize}=await import(pathToFileURL(join(directory,'workloads.mjs'))),corpus=JSON.parse(await readFile(join(directory,'corpus.json'),'utf8'));
 for(const mode of ['utf8','js-strings'])for(const optimized of [false,true]){
  const name=mode+(optimized?'-O3':''),source=join(output,name),wasm=join(source,'native.wasm');await cp(join(directory,'direct-'+mode),source,{recursive:true});
  if(optimized)await exec(optimizer,[wasm,...features,'--strip-debug','--strip-producers','--strip-target-features','-O3','-o',wasm]);
  const bytes=await readFile(wasm),compileOptions=mode==='js-strings'?{builtins:['js-string'],importedStringConstants:'walz:strings'}:{};
  const module=await WebAssembly.compile(bytes,compileOptions);
  const js=await bundle(`export {instantiate} from ${JSON.stringify(join(source,'index.js'))};`,name,output);
  const {instantiate}=await import(pathToFileURL(join(output,js.entry)));
  const p=await instantiate({getCoreModule:async()=>module});
  for(const c of corpus){const value=materialize(c.value);assert.deepEqual(reference.decode(p.api.encode(value)),value);assert.deepEqual(p.api.decode(reference.encode(value)),{tag:'ok',val:value});}
  const w=compress(bytes);report.variants[name]={js,wasm:w,total:{raw:js.raw+w.raw,gzip:js.gzip+w.gzip,brotli:js.brotli+w.brotli}};
  console.log(name+': '+report.variants[name].total.gzip+' gzip bytes');
 }
 await copy(fileURLToPath(import.meta.url),join(output,'tool.mjs'));await writeFile(join(output,'size.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
}
const [mode,first,second,third]=process.argv.slice(2);
if(mode==='capture'&&first&&!second)await capture(resolve(first));
else if(mode==='run'&&first&&second){const dir=resolve(first);await verify(dir);const {run}=await import(pathToFileURL(join(dir,'native.mjs')));await run(dir,resolve(second));}
else if(mode==='size'&&first&&second&&third)await sizes(resolve(first),resolve(second),resolve(third));
else throw new Error('usage: js-native-bench.mjs capture directory | run snapshot output | size snapshot output binaryen-133');
