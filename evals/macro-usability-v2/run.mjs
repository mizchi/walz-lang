import assert from 'node:assert/strict';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {cp,mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import {createRunDirectory} from '../macro-usability/support.mjs';
import {metrics} from '../lib/source.mjs';

const execFile=promisify(callback),root=fileURLToPath(new URL('../../',import.meta.url));
const binary=resolve(process.env.WALZ??join(root,'target/debug/walz'));
const output=await createRunDirectory(join(root,'evals/runs'),process.argv[2]);
const snapshot=join(output,'snapshot'),work=join(output,'work');
await mkdir(work,{recursive:true});
await cp(join(root,'evals/macro-usability-v2'),join(snapshot,'suite'),{recursive:true});
for(const file of ['CHEATSHEET.md','docs/user-macros.md','docs/lsp.md']){
  await mkdir(dirname(join(snapshot,file)),{recursive:true});await cp(join(root,file),join(snapshot,file));
}
const {cases,classify,describeSyntax}=await import(pathToFileURL(join(snapshot,'suite/cases.mjs')));
await mkdir(join(snapshot,'programs'));
for(const item of cases)await writeFile(join(snapshot,'programs',`${item.name}.walz`),item.source+'\n',{flag:'wx'});
await writeFile(join(snapshot,'plan.json'),JSON.stringify(cases.map(({source,...item})=>({...item,file:`programs/${item.name}.walz`})),null,2)+'\n',{flag:'wx'});
const sha=buffer=>createHash('sha256').update(buffer).digest('hex');
async function hashes(directory,prefix=''){
  const values={};for(const e of await readdir(directory,{withFileTypes:true})){
    const path=join(directory,e.name),key=prefix+e.name;
    if(e.isDirectory())Object.assign(values,await hashes(path,key+'/'));else values[key]=sha(await readFile(path));
  }return values;
}
const inputs=await hashes(snapshot);await writeFile(join(output,'inputs.json'),JSON.stringify(inputs,null,2)+'\n',{flag:'wx'});
const implementation={};
for(const directory of ['src','walz/core','crates/walz-web/src','runtime'])Object.assign(implementation,await hashes(join(root,directory),directory+'/'));
for(const file of ['Cargo.toml','Cargo.lock','build.rs','package.json','pnpm-lock.yaml'])implementation[file]=sha(await readFile(join(root,file)));
const manifest={schema:'walz-macro-authoring-v2',inputRevision:4,referenceLanguage:'en',language:'walz',extensionPrefix:'@',createdAt:new Date().toISOString(),perspective:'nonblind implementer authoring evaluation',binary,binarySha256:sha(await readFile(binary)),git:(await execFile('git',['rev-parse','HEAD'],{cwd:root})).stdout.trim(),node:process.version,platform:process.platform,architecture:process.arch,implementation};
await writeFile(join(output,'manifest.json'),JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
const commands=[];
async function command(executable,args,largeOutput){
  const start=performance.now();
  const result=await execFile(executable,args,{cwd:root,timeout:30000,maxBuffer:32*1024*1024})
    .then(r=>({code:0,...r}),e=>({code:e.killed?'timeout':e.code??'spawn-error',stdout:e.stdout??'',stderr:e.stderr??'',signal:e.signal??null}));
  const record={executable,args,milliseconds:performance.now()-start,...result};
  if(largeOutput){
    const file=`work/${largeOutput}.json.gz`,bytes=Buffer.from(record.stdout);await writeFile(join(output,file),gzipSync(bytes),{flag:'wx'});
    const log={...record,stdoutGzip:file,stdoutBytes:bytes.length,stdoutSha256:sha(bytes)};delete log.stdout;commands.push(log);
  }else commands.push(record);
  return record;
}
const results=[];
for(const item of cases){
  const source=join(snapshot,'programs',`${item.name}.walz`),checks=[];
  for(let round=0;round<3;round++){
    const record=await command(binary,['check','--json',source]);
    try{const parsed=JSON.parse(record.stdout);record.protocolValid=parsed.schema==='walz-check-v1'&&parsed.ok===(record.code===0);record.diagnostic=parsed.diagnostics?.[0];}
    catch{record.protocolValid=false;}
    checks.push(record);
  }
  let executionCorrect=false,details={};
  if(checks.every(c=>c.code===0&&c.protocolValid)&&!item.reject){
    try{
      const expansion=await command(binary,['macro-expand',source],`expansion-${item.name}`);assert.equal(expansion.code,0,expansion.stderr);
      const report=JSON.parse(expansion.stdout);details={checking:report.checking,macroDefinitions:report.definitions.length,expansions:report.expansions.length,sample:report.expansions.slice(0,2).map(e=>({name:e.name,expanded:e.expanded,inferred:e.inferred,types:e.types}))};
      const wasm=join(work,`${item.name}.wasm`),build=await command(binary,['build',source,'--target','core','-o',wasm]);assert.equal(build.code,0,build.stderr);
      details.wasmBytes=(await readFile(wasm)).length;details.wasmSha256=sha(await readFile(wasm));
      const actual={};for(const [name,expected]of Object.entries(item.expected)){
        const run=await command('wasmtime',['run','-W','gc=y,function-references=y,exceptions=y','--invoke',name,wasm]);
        assert.equal(run.code,0,run.stderr);actual[name]=run.stdout.trim();assert.equal(actual[name],expected);
      }
      const node=await command('node',[join(snapshot,'suite/run-wasm.mjs'),wasm,...Object.keys(item.expected)]);
      assert.equal(node.code,0,node.stderr);assert.deepEqual(JSON.parse(node.stdout),item.expected);details.actual=actual;executionCorrect=true;
    }catch(e){details.executionError=e.message;}
  }
  const status=classify(item,{checks,executionCorrect});
  const row={name:item.name,required:item.required,desired:item.desired,shape:item.shape,count:item.count,...status,checks,details,
    metrics:describeSyntax(item,metrics)};
  results.push(row);console.log(`${!row.passed?'FAIL':row.achieved?'PASS':'UNMET'} ${row.name}${checks[0].diagnostic?`: ${checks[0].diagnostic.code} ${checks[0].diagnostic.message}`:''}`);
}
let unchanged=true,verificationError;
try{
  assert.deepEqual(await hashes(snapshot),inputs);assert.equal(sha(await readFile(binary)),manifest.binarySha256);
  for(const [p,h]of Object.entries(implementation))assert.equal(sha(await readFile(join(root,p))),h,p);
}catch(e){unchanged=false;verificationError=e.message;}
const result={schema:'walz-macro-authoring-result-v2',passed:unchanged&&results.every(r=>r.passed),unchanged,verificationError,
  requiredPassed:results.filter(r=>r.required&&r.achieved).length,requiredTotal:results.filter(r=>r.required).length,
  unmet:results.filter(r=>!r.achieved).map(r=>r.name),failures:results.filter(r=>!r.passed).map(r=>r.name),cases:results};
await writeFile(join(output,'commands.json'),JSON.stringify(commands,null,2)+'\n',{flag:'wx'});
await writeFile(join(output,'result.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output,passed:result.passed,required:[result.requiredPassed,result.requiredTotal],unmet:result.unmet,failures:result.failures},null,2));
if(!result.passed)process.exitCode=1;
