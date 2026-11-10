// Freeze compiler output, then measure the same open-world Binaryen profiles.
import assert from 'node:assert/strict';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
import {mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRunDirectory} from '../evals/macro-usability/support.mjs';
import {cases} from './wasm-opt-cases.mjs';

const root=fileURLToPath(new URL('../',import.meta.url));
const compiler=resolve(process.env.WALZ??join(root,'target/debug/walz'));
const optimizer=process.argv[3]??'wasm-opt';
const exec=promisify(callback),sha=b=>createHash('sha256').update(b).digest('hex');
const run=async(executable,args)=>{
  const started=performance.now();
  const r=await exec(executable,args,{cwd:root,timeout:30000,maxBuffer:32*1024*1024});
  commands.push({executable,args,code:0,milliseconds:performance.now()-started,...r});return r.stdout;
};
const commands=[];
const version=(await run(optimizer,['--version'])).trim();
assert.match(version,/\b133\b/,'capture requires Binaryen 133; select its wasm-opt binary explicitly');
const output=await createRunDirectory(join(root,'target/wasm-opt'),process.argv[2]);
const save=(name,value)=>writeFile(join(output,name),value,{flag:'wx'});
await mkdir(join(output,'inputs'));await mkdir(join(output,'artifacts'));
const inputs={};
for(const item of cases){const source=item.source+'\n';await save(`inputs/${item.name}.walz`,source);inputs[`inputs/${item.name}.walz`]=sha(source);}
for(const file of ['tools/wasm-opt-cases.mjs','tools/wasm-opt-capture.mjs'])inputs[file]=sha(await readFile(join(root,file)));
const implementation={};
async function hashes(dir,prefix){for(const e of await readdir(dir,{withFileTypes:true})){const p=join(dir,e.name),key=prefix+e.name;if(e.isDirectory())await hashes(p,key+'/');else implementation[key]=sha(await readFile(p));}}
for(const dir of ['src','stdlib'])await hashes(join(root,dir),dir+'/');
for(const file of ['Cargo.toml','Cargo.lock','build.rs'])implementation[file]=sha(await readFile(join(root,file)));
const profiles=[{name:'o3',flags:['-O3']},{name:'o3-twice',flags:['-O3','-O3']}];
const features=['--enable-gc','--enable-reference-types'];
const manifest={schema:'walz-wasm-opt-v1',createdAt:new Date().toISOString(),git:(await run('git',['rev-parse','HEAD'])).trim(),compiler,binarySha256:sha(await readFile(compiler)),optimizer,version,features,profiles,inputs,implementation,node:process.version,platform:process.platform,architecture:process.arch,benchmark:{rounds:9,calls:500000,warmupCalls:50000,description:'preallocated GC values; deterministic mixed tags; JS-to-Wasm calls included'}};
await save('manifest.json',JSON.stringify(manifest,null,2)+'\n');
function codeBytes(bytes){let p=8;const leb=()=>{let n=0,s=0,b;do{b=bytes[p++];n+=(b&127)*2**s;s+=7;}while(b&128);return n;};while(p<bytes.length){const id=bytes[p++],size=leb();if(id===10)return size;p+=size;}return 0;}
const median=xs=>[...xs].sort((a,b)=>a-b)[Math.floor(xs.length/2)];
const results=[];
for(const item of cases){
  const source=join(output,'inputs',item.name+'.walz'),prefix=join(output,'artifacts',item.name);
  await run(compiler,['build',source,'--target','core','-o',prefix+'.wasm']);
  await save(`artifacts/${item.name}.wat`,await run(compiler,['wat',source]));
  const variants=[{name:'raw',path:prefix+'.wasm'}];
  for(const profile of profiles){
    const path=prefix+'.'+profile.name+'.wasm';
    const text=await run(optimizer,[prefix+'.wasm',...features,...profile.flags,'--metrics','--print','-o',path]);
    await save(`artifacts/${item.name}.${profile.name}.txt`,text);
    variants.push({name:profile.name,path});
  }
  const row={name:item.name,count:item.count,variants:[]},instances=[];
  for(const v of variants){
    const bytes=await readFile(v.path),instance=await WebAssembly.instantiate(bytes);
    assert.equal(instance.instance.exports.main(),42);
    assert.equal((await run('wasmtime',['run','-W','gc=y,function-references=y,exceptions=y','--invoke','main',v.path])).trim(),'42');
    const result={name:v.name,bytes:bytes.length,codeBytes:codeBytes(bytes),sha256:sha(bytes),actual:42};
    row.variants.push(result);
    if(item.count){
      const exports=instance.instance.exports;
      for(let i=0;i<item.count;i++)for(const n of [-2147483648,-1,0,42,2147483647])assert.equal(exports.read(exports['make'+i](n)),(n+i)|0);
      let state=20261110;const refs=Array.from({length:1024},()=>{state=(Math.imul(state,1664525)+1013904223)>>>0;const i=(state>>>24)%item.count;return exports['make'+i](42-i);});
      const call=calls=>{let sum=0;for(let i=0;i<calls;i++)sum+=exports.read(refs[i&1023]);assert.equal(sum,42*calls);};
      call(manifest.benchmark.warmupCalls);instances.push({call,result});result.samplesMs=[];
    }
  }
  for(let round=0;round<manifest.benchmark.rounds;round++)for(const v of round%2?[...instances].reverse():instances){const start=performance.now();v.call(manifest.benchmark.calls);v.result.samplesMs.push(performance.now()-start);}
  for(const v of row.variants)if(v.samplesMs)v.medianMs=median(v.samplesMs);
  results.push(row);console.log(JSON.stringify(row));
}
for(const [file,h]of Object.entries(implementation))assert.equal(sha(await readFile(join(root,file))),h,file);
for(const item of cases)assert.equal(sha(await readFile(join(output,'inputs',item.name+'.walz'))),inputs[`inputs/${item.name}.walz`]);
assert.equal(sha(await readFile(compiler)),manifest.binarySha256);
await save('commands.json',JSON.stringify(commands,null,2)+'\n');
await save('result.json',JSON.stringify({schema:'walz-wasm-opt-result-v1',passed:true,results},null,2)+'\n');
console.log(output);
