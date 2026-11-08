import assert from 'node:assert/strict';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {readFile,writeFile,mkdir,cp,readdir} from 'node:fs/promises';
import {join,resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {expenseOracle,parseReport,createRunDirectory} from './oracle.mjs';
import {probeLsp} from './lsp.mjs';
import {metrics} from '../lib/source.mjs';
import {instrument} from '../../runtime/testing/memory.mjs';
import {build as buildComponent} from '../../runtime/build.mjs';
import {createEventLoop} from '../../runtime/host.mjs';
import {transpileBytes} from '@bytecodealliance/jco-transpile';

const execFile=promisify(callback),root=fileURLToPath(new URL('../../',import.meta.url));
const binary=resolve(process.env.WLT??join(root,'target/debug/wlt'));
const id=process.argv[2];
const output=await createRunDirectory(join(root,'evals/runs'),id);
const snapshot=join(output,'snapshot'),work=join(output,'work');
await mkdir(work,{recursive:true});
await cp(join(root,'evals/usability'),join(snapshot,'evals/usability'),{recursive:true});
const docs=['README.ja.md','CHEATSHEET.md','docs/stdlib-api.md','docs/cli.md','docs/lsp.md','docs/scoped-tasks.md','docs/resources.md','docs/native-worlds.md','docs/lists.md','docs/collections.md'];
for(const file of docs){await mkdir(dirname(join(snapshot,file)),{recursive:true});await cp(join(root,file),join(snapshot,file));}
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
async function hashes(directory,prefix=''){
 const result={};for(const entry of await readdir(directory,{withFileTypes:true})){
  const path=join(directory,entry.name),name=prefix+entry.name;
  if(entry.isDirectory())Object.assign(result,await hashes(path,name+'/'));else result[name]=sha(await readFile(path));
 }return result;
}
const inputs=await hashes(snapshot);
await writeFile(join(output,'inputs.json'),JSON.stringify(inputs,null,2)+'\n');
await writeFile(join(output,'manifest.json'),JSON.stringify({schema:'wlt-usability-v1',id,createdAt:new Date().toISOString(),perspective:'nonblind implementer dogfooding; not human timing or a fresh-reader score',binary,binarySha256:sha(await readFile(binary)),node:process.version},null,2)+'\n');
const checks=[],commands=[];
async function command(args,{allowFailure=false}={}){
 const start=performance.now();const result=await execFile(binary,args,{cwd:root,maxBuffer:8*1024*1024,timeout:60000}).then(r=>({...r,code:0}),e=>({stdout:e.stdout??'',stderr:e.stderr??'',code:e.code??'spawn-error'}));
 commands.push({args,code:result.code,milliseconds:performance.now()-start,...result});
 if(!allowFailure&&result.code!==0)throw new Error(`${args[0]} failed: ${result.stderr||result.stdout}`);
 return result;
}
async function check(name,fn){const start=performance.now();try{const evidence=await fn();checks.push({name,passed:true,milliseconds:performance.now()-start,evidence});console.log(`PASS ${name}`);}catch(error){checks.push({name,passed:false,milliseconds:performance.now()-start,error:error.message});console.log(`FAIL ${name}: ${error.message}`);}}
const project=join(snapshot,'evals/usability'),expenses=join(project,'expenses');
async function build(backend,source,directory){
 if(backend==='core')await command(['build',source,'--target','core-js','-o',directory]);else await buildComponent(source,directory);
}
async function instance(backend,source,directory,options={}){
 await build(backend,source,directory);await instrument(directory,backend);let memory;
 const {instantiate}=await import(pathToFileURL(join(directory,'index.js')));
 const component=await instantiate({...options,instantiateCore:async(m,i)=>{const instance=await WebAssembly.instantiate(m,i);if(instance.exports.live)memory=instance.exports;return instance;}});
 return {...component,empty(){assert.equal(memory.live(),0);assert.ok(Object.values(component.__memory()).every(n=>n===0),JSON.stringify(component.__memory()));}};
}
const line=(category,amount)=>`{"category":${JSON.stringify(category)},"amount":${amount}}`;
const cases=[
 {name:'empty',rows:[]},
 {name:'ordinary and refund',rows:[line('food',40),line('food',2),line('work',-3)]},
 {name:'invalid rows',rows:['null','[]','{','true','{"category":2,"amount":3}','{"category":"x","amount":"3"}','{"category":"x","amount":1.5}','{"category":"x"}']},
 {name:'signed overflow is atomic',rows:[line('max',9223372036854775807n),line('max',1),line('min',-9223372036854775808n),line('min',-1)]},
 {name:'integer out of range',rows:[line('x',9223372036854775808n),line('x',-9223372036854775809n)]},
 {name:'unicode and escaped keys',rows:[line('日本😀',7),line('quote"\n',-2),line('__proto__',3),line('',0)]},
 {name:'large input',rows:Array.from({length:1500},(_,i)=>i%11===0?'invalid':line(`group-${i%29}`,i%101-50))},
];
const readme=await readFile(join(snapshot,'README.ja.md'),'utf8');
const readmeSource=readme.match(/```wat\n([\s\S]*?)```/)[1],readmePath=join(work,'readme.wlt');
await writeFile(readmePath,readmeSource);
await check('README example checks and runs',async()=>{await command(['check',readmePath]);const result=await command(['run',readmePath]);assert.equal(result.stdout.trim(),'42');return {stdout:result.stdout};});
await check('co-located test/bench can build as a JS application',async()=>{const r=await command(['build',join(expenses,'mixed.wlt'),'--target','core-js','-o',join(work,'mixed')]);return r.stdout;});
await check('CLI JSON result with the JS target',async()=>{const r=await command(['run',join(expenses,'main.wlt'),'--target','core-js']);assert.deepEqual(parseReport(JSON.parse(r.stdout)),expenseOracle([line('food',40),line('food',2),'invalid']));return {stdout:r.stdout};});
for(const backend of ['core','component'])await check(`expense oracle and cleanup: ${backend}`,async()=>{
 const c=await instance(backend,join(expenses,'main.wlt'),join(work,'expenses-'+backend));const evidence=[];
 for(const item of cases){const actual=c.api.summarize(item.rows);assert.deepEqual(parseReport(actual),expenseOracle(item.rows),item.name);c.empty();evidence.push({case:item.name,rows:item.rows.length,actual});}return evidence;
});
await check('policy change stays in one module selection',async()=>{
 const modified=join(work,'policy-change');await cp(expenses,modified,{recursive:true});
 const model=await readFile(join(modified,'model.wlt'),'utf8');
 await writeFile(join(modified,'model.wlt'),model.replace('"./parse.wlt"','"./parse-nonnegative.wlt"'));
 const c=await instance('core',join(modified,'main.wlt'),join(work,'expenses-nonnegative'));
 for(const item of cases){assert.deepEqual(parseReport(c.api.summarize(item.rows)),expenseOracle(item.rows,true),item.name);c.empty();}
 return {changedExistingLines:1,addedPolicyLines:(await readFile(join(expenses,'parse-nonnegative.wlt'),'utf8')).trimEnd().split('\n').length,cases:cases.length};
});
for(const target of ['core','core-js']){
 await check(`CLI test: ${target}`,async()=>{const r=await command(['test',join(expenses,'expenses.test.wlt'),'--target',target]);assert.match(r.stdout,/1 passed; 0 failed; 0 errors/);return r.stdout;});
 await check(`CLI bench: ${target}`,async()=>{const r=await command(['bench',join(expenses,'expenses_bench.wlt'),'--target',target,'--json','--samples','2','--iterations','1','--warmup-ms','0']);const report=JSON.parse(r.stdout);assert.equal(report.benchmarks[0].outcome.status,'measured');return {target,status:report.benchmarks[0].outcome.status};});
}
for(const backend of ['core','component'])await check(`child timeout and cancellation: ${backend}`,async()=>{
 const events=[];const c=await instance(backend,join(project,'async/main.wlt'),join(work,'async-'+backend),{eventLoop:createEventLoop({onEvent:e=>events.push(e)})});
 assert.equal(await c.api.main(),42);assert.equal(await c.api.cancel(),42);c.empty();const acquired=events.filter(e=>e.type==='timer-start'&&e.milliseconds===60000).length,disposed=events.filter(e=>e.type==='dispose'&&e.milliseconds===60000).length;assert.ok(acquired>=1);assert.equal(disposed,acquired);return {answer:42,acquired,disposed};
});
await check('LSP unsaved modules, UTF-16, navigation and repair',()=>probeLsp(binary,join(work,'unsaved-ide')));
await check('generated native resource own/borrow and async value',async()=>{
 const native=join(project,'native'),generated=join(native,'generated'),out=join(work,'native');
 await command(['bind',join(native,'api.wit'),join(native,'policy.json'),generated]);
 await mkdir(out,{recursive:true});const wasm=join(out,'native.wasm');
 await command(['build',join(native,'main.wlt'),'--target','component','--wit',join(native,'api.wit'),'--bindings',join(generated,'bindings.json'),'-o',wasm]);
 const result=await transpileBytes(await readFile(wasm),{name:'native',instantiation:'async',asyncMode:'jspi',wasiShim:false,nodejsCompat:false,base64Cutoff:0,noComponentErrorWrapping:false,bindgenEnableWasmExnref:true,multiMemory:true});
 for(const [name,bytes]of Object.entries(result.files)){const file=resolve(out,name);assert.ok(file.startsWith(out+'/'));await mkdir(dirname(file),{recursive:true});await writeFile(file,bytes);}
 await writeFile(join(out,'package.json'),'{"type":"module"}');const {instantiate}=await import(pathToFileURL(join(out,'native.js')));
 let closed=0;class Ticket{[Symbol.dispose](){closed++;}}
 const host={Ticket,open:()=>new Ticket(),size:()=>42};
 const component=await instantiate(async name=>WebAssembly.compile(await readFile(join(out,name))),{'usability:ticket/store':host});
 const api=component['usability:ticket/api'];for(let i=0;i<5;i++){assert.equal(api.run(),42);assert.equal(closed,i+1);}assert.equal(await(await api.ready()),42);
 return {run:42,future:42,closed,bindingTypes:await readFile(join(generated,'types.wlt'),'utf8')};
});
const diagnostics=[];
await check('domain diagnostics reject real mistakes without panicking',async()=>{
 const source=await readFile(join(expenses,'model.wlt'),'utf8');
 for(const [name,from,to]of [['unknown-local','$previous $amount','$prevous $amount'],['wrong-generic','(%inst $dict.get i64)','(%inst $dict.get i32)']]){
  assert.ok(source.includes(from),`diagnostic mutation target missing: ${name}`);
  const path=join(work,'bad-'+name);await cp(expenses,path,{recursive:true});await writeFile(join(path,'model.wlt'),source.replace(from,to));
  const r=await command(['check','--json',join(path,'main.wlt')],{allowFailure:true});assert.notEqual(r.code,0);assert.doesNotMatch(r.stdout+r.stderr,/panicked/);diagnostics.push({name,...r});
 }return diagnostics;
});
const sourceMetrics={};for(const file of ['expenses/main.wlt','expenses/model.wlt','expenses/parse.wlt','expenses/parse-nonnegative.wlt','async/main.wlt','native/main.wlt']){const source=await readFile(join(project,file),'utf8');const {lines,tokens,maxDepth,match,groupedLets}=metrics(source);sourceMetrics[file]={lines,tokens,maxDepth,match,groupedLets,instantiations:(source.match(/\(%inst\s/g)??[]).length,structGets:(source.match(/\(struct.get\s/g)??[]).length};}
// Native binding generation adds files; existing frozen inputs must stay exact.
for(const [file,hash]of Object.entries(inputs))assert.equal(sha(await readFile(join(snapshot,file))),hash,file);
const result={schema:'wlt-usability-result-v1',id,checks,sourceMetrics,summary:{passed:checks.filter(c=>c.passed).length,failed:checks.filter(c=>!c.passed).length}};
await writeFile(join(output,'commands.json'),JSON.stringify(commands,null,2)+'\n');
await writeFile(join(output,'result.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({directory:output,...result.summary},null,2));process.exitCode=result.summary.failed?1:0;
