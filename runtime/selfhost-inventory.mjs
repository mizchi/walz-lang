// Guest-owned specialization facts joined to independently validated Core bytes.
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join,dirname} from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {openCompiler,sourceRecords,recorderRecords,sha256} from './selfhost/profiling.mjs';
import {readProject} from './selfhost/files.mjs';
const execFile=promisify(callback),decoder=new TextDecoder('utf-8',{fatal:true});

export async function capture(input,output,compiler){
  output=resolve(output);await mkdir(dirname(output),{recursive:true});await mkdir(output);
  const seed=await openCompiler(compiler),project=await readProject(seed.api,input);
  assert.ok(!project.error,'dependency discovery must succeed: '+JSON.stringify(project.error));
  const {files}=project,sources=sourceRecords(files);
  await mkdir(join(output,'sources'));
  for(let i=0;i<files.length;i++)await writeFile(join(output,'sources',i+'.walz'),files[i].source,{flag:'wx'});
  const manifest=seed.api.inventoryProject(files);
  assert.equal(manifest.result.tag,'ok',JSON.stringify(manifest.result));
  const binary=seed.api.compileProject(files);assert.equal(binary.tag,'ok',JSON.stringify(binary.tag==='err'?binary.val:null));
  assert.ok(WebAssembly.validate(binary.val));
  await writeFile(join(output,'subject.wasm'),binary.val,{flag:'wx'});
  const {stdout}=await execFile('cargo',['run','--locked','--quiet','--example','code-inventory','--',join(output,'subject.wasm')],{maxBuffer:16*1024*1024});
  const inventory=JSON.parse(stdout);
  assert.equal(manifest.functions.length,inventory.functions.length,'source manifest must match emitted function order');
  assert.equal(manifest.result.val,inventory.functions.length);
  const declarations=new Map();
  inventory.functions.forEach((function_,i)=>{
    const f=manifest.functions[i],source=f.file<0?null:files[f.file];
    const definition=source?decoder.decode(source.source.subarray(f.start,f.end)):'';
    function_.source={file:f.file,path:source?.path??'walz:core',start:f.start,end:f.end,
      declaration:decoder.decode(f.declaration),generated:decoder.decode(f.generated),
      name:definition.match(/^\(func\s+(\$[a-zA-Z0-9_.-]+)/)?.[1]??decoder.decode(f.declaration),
      arguments:f.arguments.map(a=>decoder.decode(a).trim())};
    const key=[f.file,f.start,f.end].join(':');
    if(!declarations.has(key))declarations.set(key,{...function_.source,arguments:undefined,generated:undefined,functions:[],bodyBytes:0,instructions:0});
    const d=declarations.get(key);d.functions.push(function_.index);d.bodyBytes+=function_.bodyBytes;d.instructions+=function_.instructions;
  });
  for(let i=0;i<files.length;i++)assert.equal(sha256(await readFile(join(output,'sources',i+'.walz'))),sources[i].sha256);
  const recorder=await recorderRecords(import.meta.url);
  for(const path of ['src/code_sharing.rs','src/code_sharing/merge.rs','tools/code-inventory.rs','Cargo.lock'])
    recorder.push({path,sha256:sha256(await readFile(path))});
  const report={schema:'walz-specialization-inventory-v1',environment:{node:process.version,v8:process.versions.v8,platform:process.platform,arch:process.arch},
    compiler:seed.artifacts,recorder,sources,
    subjectSha256:sha256(binary.val),inventory,declarations:[...declarations.values()].sort((a,b)=>b.bodyBytes-a.bodyBytes)};
  await writeFile(join(output,'report.json'),JSON.stringify(report,null,2)+'\n',{flag:'wx'});
  return report;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const [input,output,compiler='target/selfhost-inventory/compiler']=process.argv.slice(2);
  if(!input||!output)throw new Error('usage: selfhost-inventory.mjs input.walz new-output-directory [compiler]');
  const report=await capture(input,output,compiler);
  console.log(JSON.stringify({functions:report.inventory.functions.length,declarations:report.declarations.length,groups:report.inventory.groups.length}));
}
