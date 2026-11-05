import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
const execFile=promisify(callback);
for(const backend of ['core','component'])test(`${backend}: WIT lists traverse import, result, future and indirect argument adapters`,{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-list-binding-'));
  try{
    const wit=join(dir,'host.wit'),policy=join(dir,'grants.json'),gen=join(dir,'gen'),source=join(dir,'main.wlt');
    const namespace='example:lists/collections@1.0.0';
    const extra=Array.from({length:14},(_,i)=>`p${i}: s32`).join(',');
    await writeFile(wit,`package example:lists@1.0.0; interface collections {
      echo: func(values: list<result<tuple<s64, string>, list<string>>>) -> list<result<tuple<s64, string>, list<string>>>;
      wide: func(a: list<s64>, b: list<string>, ${extra}) -> list<s64>;
      later: func() -> future<list<string>>;
    } world guest { import collections; }`);
    await writeFile(policy,JSON.stringify({imports:['echo','wide','later'].map(name=>({module:namespace,name,suspends:false,capability:'collections'}))}));
    await execFile('cargo',['run','--quiet','--','bind',wit,policy,gen]);
    const ty='(list (result (pair i64 text) (list text)))';
    await writeFile(source,`(module (include "gen/bindings.wlt")
      (func $echo (export "echo") (param $v ${ty}) (result ${ty}) (effects host) (call $collections.echo (local.get $v)))
      (func $wide (export "wide") (param $a (list i64)) (param $b (list text)) (result (list i64)) (effects host)
        (call $collections.wide (local.get $a) (local.get $b) ${Array.from({length:14},(_,i)=>`(i32.const ${i})`).join(' ')}))
      (func $later (export "later") (result (list text)) (effects host suspends) (future.await (list text) (call $collections.later))))`);
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');const out=join(dir,'out');
    await build(source,out,{bindings:join(gen,'bindings.json')});
    const {instantiate}=await import(pathToFileURL(join(out,'index.js')));let memory;
    const {api}=await instantiate({imports:{[namespace]:{
      echo:v=>v,
      wide:(a,b,...extra)=>{assert.deepEqual(b,['日本語']);assert.equal(extra.length,14);assert.equal(extra.at(-1),13);return a;},
      later:()=>Promise.resolve(['日本語','🦀']),
    }},instantiateCore:async(module,imports)=>{const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)memory=instance.exports;return instance;}});
    const value=[{tag:'ok',val:[42n,'日本語']},{tag:'err',val:['','🦀']}];
    assert.deepEqual(api.echo(value),value);
    assert.deepEqual(api.wide(new BigInt64Array([40n,2n]),['日本語']),new BigInt64Array([40n,2n]));
    assert.deepEqual(await api.later(),['日本語','🦀']);assert.equal(memory.live(),0);
  }finally{await rm(dir,{recursive:true,force:true});}
});
