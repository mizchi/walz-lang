import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,mkdir,rm} from 'node:fs/promises';
import {join,dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {pathToFileURL} from 'node:url';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {transpileBytes} from '@bytecodealliance/jco-transpile';
const execFile=promisify(callback);
test('native world works without WLT JS wrappers or delivery imports',{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-native-world-'));
  try{
    const input=join(dir,'main.wlt'),wit=join(dir,'world.wit'),binary=join(dir,'component.wasm');
    await writeFile(input,`(module
      (type $Item (struct (field $n i32))) (%abi.type $Item (record "n"))
      (import "test:adapter/host" "increment" (func $inc (param $n i32) (result i32)))
      (func $echo (export "api-echo") (param $v (ref $Item)) (result (ref $Item)) (%effect pure) $v)
      (func $add (export "add") (param $n i32) (result i32) (%effect host) (call $inc $n)))`);
    await writeFile(wit,`package test:adapter; interface host { increment: func(n: s32) -> s32; }
      interface api { record item { n: s32 } type unused = tuple<bool,char,s8>; echo: func(v: item) -> item; }
      world guest { import host; export api; export add: func(n: s32) -> s32; }`);
    const policy=join(dir,'policy.json');await writeFile(policy,JSON.stringify({imports:[{module:'test:adapter/host',name:'increment',params:['i32'],result:'i32',suspends:false,capability:'increment',param_names:['n']}]}));
    await execFile('cargo',['run','--locked','--quiet','--','build',input,'--target','component','--wit',wit,'--bindings',policy,'-o',binary]);
    const generated=await transpileBytes(await readFile(binary),{name:'native',instantiation:'async',wasiShim:false,nodejsCompat:false,base64Cutoff:0,noComponentErrorWrapping:false,bindgenEnableWasmExnref:true,multiMemory:true});
    for(const[name,bytes]of Object.entries(generated.files)){await mkdir(dirname(join(dir,name)),{recursive:true});await writeFile(join(dir,name),bytes);}
    await writeFile(join(dir,'package.json'),'{"type":"module"}');
    const {instantiate}=await import(pathToFileURL(join(dir,'native.js')));
    const api=await instantiate(async name=>WebAssembly.compile(await readFile(join(dir,name))),{'test:adapter/host':{increment:n=>n+1}});
    assert.deepEqual(api['test:adapter/api'].echo({n:42}),{n:42});assert.equal(api.add(41),42);
  }finally{await rm(dir,{recursive:true,force:true});}
});
