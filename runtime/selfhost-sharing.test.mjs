import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,writeFile,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instrument,collect} from './testing/memory.mjs';
import {capture} from './selfhost-inventory.mjs';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
const execFile=promisify(callback);
const encoder=new TextEncoder(),decoder=new TextDecoder();
for(const backend of ['core','component'])test(`${backend}: specialization inventory is request-local and preserves validation`,{timeout:240000},async()=>{
  const root=await mkdtemp(join(tmpdir(),'walz-sharing-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('examples/selfhost/inventory.walz',root);await instrument(root,backend);
    let arena;
    const instance=await(await import(pathToFileURL(join(root,'index.js')))).instantiate({instantiateCore:async(module,imports)=>{
      const value=await WebAssembly.instantiate(module,imports);if(value.exports.live)arena=value.exports;return value;
    }});
    const api=instance.api,clean=()=>{assert.equal(arena.live(),0);assert.ok(Object.values(instance.__memory()).every(n=>n===0));};
    const source=`(module (@use "walz:core" (type $option)) ;; 日本語
      (func $answer (@type.params $T) (result i32) 42)
      (func $entry (export "f") (result i32)
        (+ (call $answer<i32>) (+ (call $answer<i64>) (call $answer<i32>)))))`;
    const files=[{path:'main.walz',source:encoder.encode(source)}],definition=f=>decoder.decode(files[0].source.subarray(f.start,f.end));
    const report=api.inventoryProject(files);clean();assert.equal(report.result.tag,'ok');
    const answers=report.functions.filter(f=>f.file===0&&definition(f).startsWith('(func $answer'));
    assert.equal(answers.length,2,'same type arguments reuse their instance: '+JSON.stringify(report));
    assert.deepEqual(answers.map(f=>f.arguments.map(a=>decoder.decode(a).trim())).sort(),[['i32'],['i64']]);
    assert.ok(answers.every(f=>f.file===0&&definition(f).startsWith('(func $answer')));
    assert.equal(new Set(report.functions.map(f=>decoder.decode(f.generated))).size,report.functions.length);
    assert.deepEqual(api.inventoryProject(files),report,'deterministic request-local inventory');clean();
    const binary=api.compileProject(files);clean();assert.equal(binary.tag,'ok');
    assert.equal((await WebAssembly.instantiate(binary.val)).instance.exports.f(),126);
    const invalid=source.replace('(@type.params $T)','(@type.params $T) (@where (Logical $T))');
    assert.equal(api.inventoryProject([{path:'bad.walz',source:encoder.encode(invalid)}]).result.tag,'err');clean();
    assert.deepEqual(api.inventoryProject(files),report);await collect();clean();
    const entry=join(root,'input.walz'),output=join(root,'capture');await writeFile(entry,source);
    const saved=await capture(entry,output,root);
    assert.equal(saved.declarations.find(d=>d.name==='$answer').functions.length,2);
    assert.equal(saved.inventory.groups.length,1);
    assert.equal(saved.inventory.functions.length,report.functions.length);
    const original=await readFile(join(output,'report.json'));
    await assert.rejects(capture(entry,output,root),{code:'EEXIST'});
    assert.deepEqual(await readFile(join(output,'report.json')),original);
    const identity=`(module
      (func $left (result i32) (i32.const 42))
      (func $right (result i32) (i32.const 42))
      (func $a (export "a") (result i32) (i32.const 42))
      (func $b (export "b") (result i32) (i32.const 42))
      (func $trap-a (param $n i32) (result i32) (i32.div_s (i32.const 1) (local.get $n)))
      (func $trap-b (param $n i32) (result i32) (i32.div_s (i32.const 1) (local.get $n)))
      (func $trap (export "trap") (param $n i32) (result i32) (call $trap-b (local.get $n)))
      (func $f (export "f") (result i32) (i32.add (call $left) (call $right))))`;
    const built=api.compileProject([{path:'identity.walz',source:encoder.encode(identity)}]);assert.equal(built.tag,'ok');clean();
    const before=join(root,'before.wasm'),after=join(root,'after.wasm');await writeFile(before,built.val);
    await execFile('cargo',['run','--locked','--quiet','--example','code-inventory','--',before,after]);
    const shared=await readFile(after);assert.ok(shared.length<built.val.length);
    for(const wasm of [built.val,shared]){
      const {instance}=await WebAssembly.instantiate(wasm);
      assert.equal(instance.exports.f(),84);assert.notEqual(instance.exports.a,instance.exports.b);
      assert.throws(()=>instance.exports.trap(0),WebAssembly.RuntimeError);
      assert.equal(instance.exports.trap(1),1);
    }
  }finally{await rm(root,{recursive:true,force:true});}
});
