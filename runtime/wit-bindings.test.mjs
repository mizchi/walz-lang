import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { execFile as callback } from 'node:child_process';
import { promisify } from 'node:util';
const execFile = promisify(callback);

for (const backend of ['core', 'component']) test(`${backend}: generated WIT bindings preserve composite values and suspension`, {timeout:60000}, async()=>{
  const dir = await mkdtemp(join(tmpdir(),'walz-wit-'));
  try {
    const wit = join(dir,'host.wit'), grants = join(dir,'policy.json'), generated = join(dir,'gen');
    const many = Array.from({length:17},(_,i)=>`p${i}: s32`).join(',');
    await writeFile(wit, `package example:host@1.0.0; interface service {
      echo: func(value: result<tuple<s64, option<string>>, tuple<f32, list<u8>>>) -> result<tuple<s64, option<string>>, tuple<f32, list<u8>>>;
      later: func(value: s64) -> future<result<s64, s32>>;
      wait: async func(value: s64) -> option<string>;
      many: func(${many}) -> s32;
    } world guest { import service; }`);
    const namespace = 'example:host/service@1.0.0';
    await writeFile(grants,JSON.stringify({imports:['echo','later','wait','many'].map(name=>({module:namespace,name,suspends:name==='wait',capability:'test'}))}));
    await execFile('cargo',['run','--quiet','--','bind',wit,grants,generated]);
    const ty = "(@result (@tuple i64 (@option @string)) (@tuple f32 @bytes))";
    const source = join(dir,'main.walz');
    await writeFile(source, `(module (@include "gen/bindings.walz")
      (func $echo (export "echo") (param $v ${ty}) (result ${ty}) (@effect host) (call $service.echo (local.get $v)))
      (func $later (export "later") (param $v i64) (result (@result i64 i32)) (@effect host suspends)
        (@future.await (@result i64 i32) (call $service.later (local.get $v))))
      (func $wait (export "wait") (result (@option @string)) (@effect host suspends) (call $service.wait (i64.const 42)))
      (func $many (export "many") (result i32) (@effect host) (call $service.many ${Array.from({length:17},(_,i)=>`(i32.const ${i})`).join(' ')})))`);
    const {build} = await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    const out = join(dir,'out');
    await build(source,out,{bindings:join(generated,'bindings.json')});
    const runtime = await import(pathToFileURL(join(out,'index.js')));
    const {api} = await runtime.instantiate({imports:{[namespace]:{
      echo: value=>value,
      later: value=>Promise.resolve({tag:'ok',val:value}),
      wait: async value=>`日本語${value}`,
      many: (...values)=>values.reduce((a,b)=>a+b,0),
    }}});
    for(const value of [{tag:'ok',val:[42n,'日本語']},{tag:'ok',val:[-1n,undefined]},{tag:'err',val:[1.25,new Uint8Array([0,255])]}])
      assert.deepEqual(api.echo(value),value);
    assert.deepEqual(await api.later(42n),{tag:'ok',val:42n});
    assert.equal(await api.wait(),'日本語42');
    assert.equal(api.many(),136);
  } finally { await rm(dir,{recursive:true,force:true}); }
});
