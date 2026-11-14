// Fixed-point verification of the supported compiler, not Rust feature parity.
import assert from 'node:assert/strict';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {resolve,dirname,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {openCompiler,sourceRecords,recorderRecords,sha256} from './selfhost/profiling.mjs';
import {readProject} from './selfhost/files.mjs';
import {openStage} from './selfhost/staged.mjs';
import {programs,projects,invalid,badProjects} from './selfhost-front/corpus.mjs';
const encode=s=>new TextEncoder().encode(s);
const records=files=>files.map(([path,source])=>({path,source:encode(source)}));
const cases=()=>({
  positive:[...programs.map(([name,source,expected])=>[name,[['main.walz',source]],expected]),...projects],
  negative:[...invalid.map(([name,source,code])=>[name,[['main.walz',source]],code]),...badProjects],
});
export async function verifyStages(seedPath,output){
  output=resolve(output);
  await mkdir(dirname(output),{recursive:true});
  await mkdir(output); // Never replace a previous source or measurement capture.
  const seed=await openCompiler(seedPath),{files,error}=await readProject(seed.api,'examples/selfhost/staged.walz');
  assert.ifError(error&&new Error(JSON.stringify(error)));
  const subject=files=>({sources:sourceRecords(files),labels:files.map(f=>f.path)});
  const inputs=subject(files),inputHash=sha256(JSON.stringify(inputs)),corpus=cases();
  const corpusText=JSON.stringify(corpus,(_,v)=>typeof v==='bigint'?{i64:v.toString()}:v);
  await mkdir(join(output,'inputs'));
  for(let i=0;i<files.length;i++)await writeFile(join(output,'inputs',`${i}.walz`),files[i].source,{flag:'wx'});
  await writeFile(join(output,'inputs.json'),JSON.stringify({...inputs,sha256:inputHash},null,2)+'\n',{flag:'wx'});
  await writeFile(join(output,'cases.json'),corpusText+'\n',{flag:'wx'});
  const report={schema:'walz-selfhost-stages-v1',status:'started',
    environment:{node:process.version,v8:process.versions.v8,platform:process.platform,arch:process.arch},
    seed:seed.artifacts,recorder:[...await recorderRecords(import.meta.url),
      {path:'runtime/selfhost/staged.mjs',sha256:sha256(await readFile(new URL('./selfhost/staged.mjs',import.meta.url)))}],sources:inputs.sources,
    inputsSha256:inputHash,casesSha256:sha256(corpusText+'\n'),stages:[],binaryEquality:false,
    verified:{positive:0,negative:0},error:null};
  async function save(status){report.status=status;await writeFile(join(output,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(status);}
  async function compile(api,label,input=files){
    await save('compiling-'+label);
    const hash=sha256(JSON.stringify(subject(input)));
    if(input===files)assert.equal(hash,inputHash,'source inputs changed');
    const start=performance.now(),result=api.compileProject(input),elapsedMs=performance.now()-start;
    assert.equal(sha256(JSON.stringify(subject(input))),hash,'compiler mutated source inputs');
    if(result.tag!=='ok'){report.error={stage:label,...result.val};await save('failed');throw new Error(JSON.stringify(report.error));}
    assert.ok(WebAssembly.validate(result.val),label+' must independently validate');
    await writeFile(join(output,label+'.wasm'),result.val,{flag:'wx'});
    report.stages.push({name:label,bytes:result.val.length,sha256:sha256(result.val),elapsedMs,inputsSha256:hash});
    return result.val;
  }
  try{
    // The Rust-built seed uses Core+JSPI transport. Both self-compiled stages
    // expose the same small Core/GC test bridge and receive identical sources.
    const first=await compile(seed.api,'stage1'),stage1=await openStage(first);
    const second=await compile(stage1,'stage2'),stage2=await openStage(second);
    assert.deepEqual(second,first,'self-compiled stage binaries must match byte for byte');
    report.binaryEquality=true;
    const portable=await readProject(seed.api,'examples/selfhost/portable.walz');
    assert.ifError(portable.error&&new Error(JSON.stringify(portable.error)));
    await mkdir(join(output,'portable-inputs'));
    for(let i=0;i<portable.files.length;i++)await writeFile(join(output,'portable-inputs',`${i}.walz`),portable.files[i].source,{flag:'wx'});
    await writeFile(join(output,'portable-inputs.json'),JSON.stringify(subject(portable.files),null,2)+'\n',{flag:'wx'});
    const portable1=await compile(stage1,'portable1',portable.files),portable2=await compile(stage2,'portable2',portable.files);
    assert.deepEqual(portable2,portable1,'portable compiler output must match too');
    assert.deepEqual(WebAssembly.Module.exports(await WebAssembly.compile(portable1)).map(e=>[e.name,e.kind]),
      ['compile','check','compile-project','check-project','imports'].map(name=>[name,'function']));
    await save('verifying-cases');
    for(const [name,p,expected] of corpus.positive){
      const input=records(p),reference=seed.api.compileProject(input);
      assert.equal(reference.tag,'ok',name+' seed '+JSON.stringify(reference.tag==='err'?reference.val:null));
      for(const [label,api] of [['stage1',stage1],['stage2',stage2]]){
        const result=api.compileProject(input);
        assert.equal(result.tag,'ok',label+' '+name+' '+JSON.stringify(result.tag==='err'?result.val:null));
        assert.deepEqual(result.val,reference.val,label+' '+name+' emitted bytes');
        assert.ok(WebAssembly.validate(result.val),label+' '+name);
        const {instance}=await WebAssembly.instantiate(result.val);
        if(expected==='trap')assert.throws(()=>instance.exports.f(),WebAssembly.RuntimeError,name);
        else assert.equal(instance.exports.f(),expected,label+' '+name);
      }
      report.verified.positive++;
      if(report.verified.positive%20===0)await save('verifying-cases');
    }
    for(const [name,p,code] of corpus.negative){
      const input=records(p),reference=seed.api.compileProject(input);
      assert.equal(reference.tag,'err',name+' seed');assert.equal(reference.val.code,code,name+' seed code');
      for(const [label,api] of [['stage1',stage1],['stage2',stage2]])
        assert.deepEqual(api.compileProject(input),reference,label+' '+name+' original diagnostics');
      report.verified.negative++;
      if(report.verified.negative%20===0)await save('verifying-cases');
    }
    // Verify the retained capture as well as the in-memory subjects.
    for(let i=0;i<files.length;i++)assert.equal(sha256(await readFile(join(output,'inputs',`${i}.walz`))),sha256(files[i].source));
    for(let i=0;i<portable.files.length;i++)assert.equal(sha256(await readFile(join(output,'portable-inputs',`${i}.walz`))),sha256(portable.files[i].source));
    assert.equal(sha256(await readFile(join(output,'cases.json'))),report.casesSha256);
    await save('complete');return report;
  }catch(error){report.error??={message:String(error.stack??error)};await save('failed');throw error;}
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const [seed,output]=process.argv.slice(2);
  if(!seed||!output)throw new Error('usage: selfhost-stage.mjs seed-profile-compiler new-output-directory');
  await verifyStages(seed,output);
}
