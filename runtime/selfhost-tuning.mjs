// Freeze inputs once, then compare isolated compiler artifacts across iterations.
import assert from 'node:assert/strict';
import {mkdir,writeFile,readFile,readdir} from 'node:fs/promises';
import {resolve,relative,dirname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {fields} from './selfhost-front/scaling.mjs';
import {genericLists} from './selfhost-front/tuning.mjs';
import {openCompiler,sha256} from './selfhost/profiling.mjs';
import {readProject} from './selfhost/files.mjs';
const exec=promisify(callback), root=fileURLToPath(new URL('..',import.meta.url));
const schema='walz-selfhost-tuning-inputs-v1';
const safe=(base,path)=>{
  const target=resolve(base,path),local=relative(base,target);
  assert.ok(local&&!local.startsWith('..')&&!local.startsWith('/'),'input path must stay inside its snapshot');
  return target;
};
async function fresh(path){await mkdir(dirname(path),{recursive:true});await mkdir(path);}

export async function verifyInputs(directory){
  const path=join(directory,'inputs.json'),bytes=await readFile(path),manifest=JSON.parse(bytes);
  assert.equal(manifest.schema,schema);
  const expected=new Set(['inputs.json']);
  for(const c of manifest.cases){
    assert.ok(c.files.some(f=>f.path===c.entry),'entry must be captured');
    for(const f of c.files){
      assert.ok(!expected.has(f.path),'duplicate captured file');expected.add(f.path);
      assert.equal(sha256(await readFile(safe(directory,f.path))),f.sha256,'captured input changed');
    }
  }
  async function walk(path){
    for(const entry of await readdir(path,{withFileTypes:true})){
      const child=join(path,entry.name);
      if(entry.isDirectory())await walk(child);
      else {assert.ok(entry.isFile(),'only regular input files are allowed');assert.ok(expected.delete(relative(directory,child)),'unexpected input file');}
    }
  }
  await walk(directory);assert.equal(expected.size,0);
  return {manifest,hash:sha256(bytes)};
}
async function prepare(compiler,directory,sourceRoot=root){
  const {api,artifacts}=await openCompiler(compiler);
  await fresh(directory);
  const cases=[];
  async function capture(name,entry,files){
    const records=[];
    for(const file of files){
      const path=join(name,relative(sourceRoot,file.path));
      const target=safe(directory,path);await mkdir(dirname(target),{recursive:true});
      await writeFile(target,file.source,{flag:'wx'});records.push({path,sha256:sha256(file.source)});
    }
    cases.push({name,entry:join(name,relative(sourceRoot,entry)),files:records});
  }
  for(const [name,source] of [['fields',fields(1000)],['generic-lists',genericLists(128)]]){
    const path=join(sourceRoot,'main.walz');await capture(name,path,[{path,source:new TextEncoder().encode(source)}]);
  }
  for(const [name,input] of [
    ['project','examples/selfhost/project/main.walz'],['number','walz/parser/number.walz'],
    ['graph','walz/frontend/graph.walz'],['compiler','examples/selfhost/portable.walz'],
  ]){
    const entry=resolve(sourceRoot,input),{files,error}=await readProject(api,entry);
    assert.ifError(error&&new Error(JSON.stringify(error)));await capture(name,entry,files);
  }
  await writeFile(join(directory,'inputs.json'),JSON.stringify({schema,discoveryCompiler:artifacts,cases},null,2)+'\n',{flag:'wx'});
  await verifyInputs(directory);
}
async function compare(before,after,inputs,output,samples){
  const {manifest,hash}=await verifyInputs(inputs);await fresh(output);
  const cases=[];let failed=false;
  for(const c of manifest.cases){
    const path=join(output,c.name+'.json');let commandFailed=false;
    try{await exec(process.execPath,[fileURLToPath(new URL('./selfhost-profile-compare.mjs',import.meta.url)),before,after,safe(inputs,c.entry),path,samples],{cwd:root});}
    catch{commandFailed=true;}
    const report=JSON.parse(await readFile(path));
    const regression=commandFailed||(report.before.result.tag==='ok'&&!report.binaryEquality);
    failed ||= regression;
    cases.push({name:c.name,report:c.name+'.json',binaryEquality:report.binaryEquality,
      speedup:report.speedup,beforeMs:report.before.medianMs,afterMs:report.after.medianMs,
      beforeNodes:report.before.stats?.syntaxNodes,afterNodes:report.after.stats?.syntaxNodes,
      atomHits:report.after.stats?.syntaxAtomHits??null,
      beforePhase:report.before.phase,afterPhase:report.after.phase,regression});
    console.log(c.name+': '+(report.speedup===null?'timing withheld':report.speedup.toFixed(2)+'x'));
  }
  await verifyInputs(inputs);
  await writeFile(join(output,'comparison.json'),JSON.stringify({schema:'walz-selfhost-tuning-v1',inputsSha256:hash,cases},null,2)+'\n',{flag:'wx'});
  if(failed)process.exitCode=1;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const [command,...args]=process.argv.slice(2);
  if(command==='prepare'&&(args.length===2||args.length===3))await prepare(args[0],resolve(args[1]),args[2]?resolve(args[2]):root);
  else if(command==='compare'&&args.length>=4&&args.length<=5){
    const [before,after,inputs,output,samples='9']=args;
    assert.ok(Number.isInteger(Number(samples))&&Number(samples)>=3&&Number(samples)<=99);
    await compare(before,after,resolve(inputs),resolve(output),samples);
  }else throw new Error('usage: selfhost-tuning.mjs prepare compiler new-inputs [source-root] | compare before after inputs new-output [samples:3..99]');
}
