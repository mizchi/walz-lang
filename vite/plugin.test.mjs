import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, readFile, writeFile, rm, readdir, mkdir, symlink} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {build, createServer} from 'vite';
import walz from './index.mjs';

const compiler = resolve('target/debug/walz');
const execFile = promisify(callback);
const program = `(module
  (@use "walz:core" (type $string) (type $bytes) (type $future) (type $stream) (type $list))
  (@use "./helper.walz" (func $answer))
  (func $main (export "main") (result i32) (@effect pure) (call $answer))
  (func $echo (export "echo") (param $text $string) (result $string) (@effect pure) $text)
  (func $bytes (export "bytes") (param $text $bytes) (result $bytes) (@effect pure) $text)
  (func $wide (export "add-one") (param $n i64) (result i64) (@effect pure) (+ $n 1))
  (func $future (export "future") (result (@inst $future i64)) (@effect pure) (@future.ready i64 42))
  (func $stream (export "stream") (result (@inst $stream i32)) (@effect pure)
    (@stream.cons i32 21 (@stream.cons i32 21 (@stream.empty i32))))
  (func $list (export "list") (param $values (@inst $list i32)) (result (@inst $list i32)) (@effect pure) $values)
  (func $wait (export "wait") (param $value (@inst $future i32)) (result i32) (@effect suspends)
    (@future.await i32 (@move $value))))`;
const helper = '(module (@export (func $answer)) (func $answer (result i32) (@effect pure) 42))';

async function fixture(fn) {
  const root = await mkdtemp(join(tmpdir(), 'walz vite-'));
  try {
    await writeFile(join(root, 'program.walz'), program);
    await writeFile(join(root, 'helper.walz'), helper);
    await writeFile(join(root, 'entry.mjs'), `export * from './program.walz';export {default as init} from './program.walz?init';`);
    await fn(root);
  } finally { await rm(root, {recursive:true, force:true}); }
}

async function bundle(root, options={}) {
  await build({root, configFile:false, logLevel:'silent', plugins:[walz({compiler, ...options})],
    build:{ssr:'entry.mjs', outDir:'dist', target:'esnext'}});
  return import(pathToFileURL(join(root, 'dist/entry.mjs')).href);
}

test('ESM names, rich ABI values, Promise exports and explicit fresh instances work in SSR builds', async () => {
  await fixture(async root => {
    const api = await bundle(root, {dts:true});
    assert.equal(api.main(), 42);
    assert.equal(api.echo('漢字 😀'), '漢字 😀');
    assert.deepEqual(api.bytes(new Uint8Array([0,255,42])), new Uint8Array([0,255,42]));
    assert.equal(api.addOne(41n), 42n);
    assert.equal(await api.wait(Promise.resolve(42)), 42);
    const future = api.future();
    assert.equal(future instanceof Promise, false);
    assert.equal(await future, 42n);
    await future[Symbol.asyncDispose]();
    const stream = api.stream();
    assert.deepEqual(await stream.next(), {value:21,done:false});
    assert.deepEqual(await stream.return(), {done:true});
    assert.deepEqual(api.list([1,-1,2147483647]), new Int32Array([1,-1,2147483647]));
    const instances = [];
    const options = {instantiateCore:async (module, imports) => {
      const instance = await WebAssembly.instantiate(module, imports); instances.push(instance); return instance;
    }};
    const first = await api.init(options), second = await api.init(options);
    assert.equal(first.api.main(), 42); assert.equal(second.api.main(), 42);
    assert.equal(new Set(instances).size, 4);
    assert.notEqual(first.api.main, second.api.main);
    const declaration = await readFile(join(root, 'program.d.walz.ts'), 'utf8');
    assert.match(declaration, /WalzExports/);
    assert.match(declaration, /bigint/);
    assert.match(declaration, /Promise/);
    await writeFile(join(root, 'check.ts'), `import {main, addOne, echo, bytes, wait, future, stream, list} from './program.walz';
      import type {WalzExports} from './program.walz';
      import init from './program.walz?init';
      const n:number=main(), b:bigint=addOne(41n), s:string=echo('text'), data:Uint8Array=bytes(new Uint8Array());
      const p:Promise<number>=wait(Promise.resolve(42));
      const f=future(); const futureValue:bigint=await f; await f[Symbol.asyncDispose]();
      // @ts-expect-error returned Future is PromiseLike, not a native Promise
      f.catch(()=>{});
      const s2=stream(); const step=await s2.next();await s2.return();
      const numbers:Int32Array=list([1,2] as const);
      // @ts-expect-error i64 requires bigint
      addOne(41);
      // @ts-expect-error unknown named export
      import {missing} from './program.walz';
      const instance=await init<WalzExports>();
      const result:bigint=await instance.start('addOne',[41n]).result;
      // @ts-expect-error held Future task results cannot be treated as their resolved payload
      const held:bigint=await instance.start('future',[]).result;
      // @ts-expect-error wrong task argument
      instance.start('addOne',[41]);`);
    await mkdir(join(root, 'node_modules/@mizchi'), {recursive:true});
    await symlink(resolve('.'), join(root, 'node_modules/@mizchi/walz-runtime'), 'dir');
    await writeFile(join(root, 'tsconfig.json'), JSON.stringify({compilerOptions:{
      strict:true,noEmit:true,target:'esnext',module:'esnext',moduleResolution:'bundler',allowArbitraryExtensions:true,
      types:['@mizchi/walz-runtime/vite/client'],
    },include:['check.ts']}));
    const checked = await execFile('node', [resolve('node_modules/typescript/bin/tsc'), '-p', join(root, 'tsconfig.json')]).catch(error=>error);
    assert.equal(checked.code ?? 0, 0, checked.stdout || checked.stderr);
  });
});

test('relative ESM hosts are linked from the Walz file; ?init accepts explicitly supplied hosts', async () => {
  await fixture(async root => {
    await writeFile(join(root, 'host.mjs'), 'export const increment = n => n + 1;');
    await writeFile(join(root, 'bindings.json'), JSON.stringify({imports:[{
      module:'./host.mjs', name:'increment', params:['i32'], result:'i32', suspends:false, capability:'increment',
    }]}));
    await writeFile(join(root, 'program.walz'), `(module
      (import "./host.mjs" "increment" (func $increment (param $n i32) (result i32)))
      (func $main (export "main") (result i32) (@effect host) (call $increment 41)))`);
    const api = await bundle(root, {bindings:'bindings.json'});
    assert.equal(api.main(), 42);
    const custom = await api.init({imports:{'./host.mjs':{increment:n=>n+9}}});
    assert.equal(custom.api.main(), 50);
  });
});

test('compiler failures retain source diagnostics and do not fall back to stale code or overwrite declarations', async () => {
  await fixture(async root => {
    await writeFile(join(root, 'program.walz'), '(module (func $bad (result i32) $missing))');
    await assert.rejects(bundle(root), /unbound local.*missing|unknown.*missing/s);
    await writeFile(join(root, 'program.walz'), program);
    await writeFile(join(root, 'program.d.walz.ts'), 'export const userFile = 42;');
    await assert.rejects(bundle(root, {dts:true}), /declaration.*overwrite|overwrite.*declaration/i);
  });
});

test('dev dependency edits invalidate ESM and factory modules and recover after errors', async () => {
  await fixture(async root => {
    const server = await createServer({root, configFile:false, logLevel:'silent',
      plugins:[walz({compiler, dts:true})], server:{port:0}});
    try {
      await server.listen();
      assert.equal((await server.ssrLoadModule('/entry.mjs')).main(), 42);
      const next = helper.replace(' 42)', ' 43)');
      await writeFile(join(root, 'helper.walz'), next);
      await server.watcher.emit('change', join(root, 'helper.walz'));
      await new Promise(resolve => setTimeout(resolve, 150));
      assert.equal((await server.ssrLoadModule('/entry.mjs')).main(), 43);
      await writeFile(join(root, 'helper.walz'), next.replace(' 43)', ' $missing)'));
      server.watcher.emit('change', join(root, 'helper.walz'));
      await new Promise(resolve => setTimeout(resolve, 150));
      await assert.rejects(server.ssrLoadModule('/entry.mjs'), error => {
        assert.match(error.message, /missing/);
        assert.ok(error.loc?.file?.endsWith('/helper.walz'), JSON.stringify(error.loc));
        assert.ok(error.loc.line > 0);
        return true;
      });
      await writeFile(join(root, 'helper.walz'), helper);
      server.watcher.emit('change', join(root, 'helper.walz'));
      await new Promise(resolve => setTimeout(resolve, 150));
      assert.equal((await server.ssrLoadModule('/entry.mjs')).main(), 42);
    } finally { await server.close(); }
    const cache = join(root, 'node_modules/.vite');
    assert.equal((await readdir(cache).catch(()=>[])).some(name=>name.startsWith('walz-')), false);
  });
});
