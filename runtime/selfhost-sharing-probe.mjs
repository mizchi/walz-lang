// Optional, version-pinned postprocessing experiment; never part of compilation.
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join,dirname} from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {openStage} from './selfhost/staged.mjs';
import {sha256} from './selfhost/profiling.mjs';
import {programs,projects,invalid,badProjects} from './selfhost-front/corpus.mjs';
const execFile=promisify(callback),encode=s=>new TextEncoder().encode(s);
const records=files=>files.map(([path,source])=>({path,source:encode(source)}));

export async function probe(input,output,optimizer){
  output=resolve(output);await mkdir(dirname(output),{recursive:true});await mkdir(output);
  const bytes=await readFile(input),reference=await openStage(bytes);
  const version=(await execFile(optimizer,['--version'])).stdout.trim();
  assert.match(version,/\b133\b/,'this experiment is pinned to Binaryen 133');
  const snapshot=join(output,'before.wasm');await writeFile(snapshot,bytes,{flag:'wx'});
  const cases={positive:[...programs.map(([name,source,expected])=>[name,[['main.walz',source]],expected]),...projects],
    negative:[...invalid.map(([name,source,code])=>[name,[['main.walz',source]],code]),...badProjects]};
  const caseText=JSON.stringify(cases,(_,value)=>typeof value==='bigint'?{i64:value.toString()}:value)+'\n';
  await writeFile(join(output,'cases.json'),caseText,{flag:'wx'});
  const report={schema:'walz-code-sharing-probe-v2',status:'started',
    environment:{node:process.version,v8:process.versions.v8,platform:process.platform,arch:process.arch},
    optimizer:{version,sha256:sha256(await readFile(optimizer))},inputSha256:sha256(bytes),casesSha256:sha256(caseText),
    recorderSha256:sha256(await readFile(new URL(import.meta.url))),candidates:[],recorder:[]};
  await mkdir(join(output,'recorders'));
  for(const path of ['runtime/selfhost-sharing-probe.mjs','runtime/selfhost/staged.mjs','src/code_sharing.rs','src/code_sharing/merge.rs','tools/code-inventory.rs','Cargo.lock']){
    const source=await readFile(path),index=report.recorder.length;
    report.recorder.push({path,sha256:sha256(source),snapshot:'recorders/'+index});
    await writeFile(join(output,'recorders',String(index)),source,{flag:'wx'});
  }
  const save=()=>writeFile(join(output,'report.json'),JSON.stringify(report,null,2)+'\n');
  await save();
  async function inventory(path){
    const {stdout}=await execFile('cargo',['run','--locked','--quiet','--example','code-inventory','--',path],{maxBuffer:16*1024*1024});
    return JSON.parse(stdout);
  }
  const before=await inventory(snapshot);report.before={bytes:bytes.length,codeBytes:before.codeSectionBytes,functions:before.functions.length,gcTypes:before.gcTypes};
  try{
    for(const name of ['exact','binaryen-roundtrip','binaryen-exact','binaryen-similar']){
      const path=join(output,name+'.wasm');
      const command=name==='exact'
        ? ['cargo',['run','--locked','--quiet','--example','code-inventory','--',snapshot,path]]
        : [optimizer,[snapshot,'--enable-gc','--enable-reference-types','--enable-bulk-memory',
          ...(name==='binaryen-roundtrip'?[]:[name==='binaryen-exact'?'--duplicate-function-elimination':'--merge-similar-functions']),'-o',path]];
      await execFile(...command,{maxBuffer:16*1024*1024});
      const wasm=await readFile(path),api=await openStage(wasm),stats=await inventory(path);
      const candidate={name,command,sha256:sha256(wasm),bytes:wasm.length,codeBytes:stats.codeSectionBytes,
        functions:stats.functions.length,gcTypes:stats.gcTypes,verified:{positive:0,negative:0}};
      report.candidates.push(candidate);await save();
      // Preserve the export alias relation as observable through the JS API.
      const instantiate=async binary=>(await WebAssembly.instantiate(binary)).instance.exports;
      const a=await instantiate(bytes),b=await instantiate(wasm),names=Object.keys(a);
      assert.deepEqual(Object.keys(b),names);
      for(const x of names)for(const y of names)assert.equal(a[x]===a[y],b[x]===b[y],name+' export identity');
      for(const [label,files,expected] of cases.positive){
        const input=records(files),before=reference.compileProject(input),after=api.compileProject(input);
        assert.equal(before.tag,'ok',label);assert.deepEqual(after,before,name+' '+label+' output');
        assert.ok(WebAssembly.validate(after.val),label);
        const {instance}=await WebAssembly.instantiate(after.val);
        if(expected==='trap')assert.throws(()=>instance.exports.f(),WebAssembly.RuntimeError,label);
        else assert.equal(instance.exports.f(),expected,label);
        candidate.verified.positive++;
      }
      for(const [label,files,code] of cases.negative){
        const input=records(files),before=reference.compileProject(input),after=api.compileProject(input);
        assert.equal(before.tag,'err',label);assert.equal(before.val.code,code,label);
        assert.deepEqual(after,before,name+' '+label+' diagnostic');candidate.verified.negative++;
      }
      await save();
    }
    assert.equal(sha256(await readFile(snapshot)),report.inputSha256);
    assert.equal(sha256(await readFile(join(output,'cases.json'))),report.casesSha256);
    report.status='complete';await save();return report;
  }catch(error){report.status='failed';report.error=String(error.stack??error);await save();throw error;}
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const [input,output,optimizer]=process.argv.slice(2);
  if(!input||!output||!optimizer)throw new Error('usage: selfhost-sharing-probe.mjs staged.wasm new-output-directory wasm-opt-133');
  console.log(JSON.stringify((await probe(input,output,resolve(optimizer))).candidates));
}
