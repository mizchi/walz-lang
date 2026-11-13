import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { execFile as callback } from "node:child_process";
import { instrument, collect } from "./testing/memory.mjs";
const encoder = new TextEncoder(), exec = promisify(callback);
const bytes = source => encoder.encode(source);
const operators = [["i32.add", (a,b) => (a+b)|0], ["i32.sub", (a,b) => (a-b)|0],
  ["i32.mul", Math.imul], ["i32.xor", (a,b) => a^b], ["i32.shl", (a,b) => a<<b],
  ["i32.shr_u", (a,b) => (a>>>b)|0], ["i32.lt_s", (a,b) => +(a<b)]];
function generatedCases() {
  let state = 0x77ab1234;
  const random = () => { state ^= state<<13; state ^= state>>>17; state ^= state<<5; return state|0; };
  function expr(depth) {
    if (!depth || (random() & 3) === 0) { const value = random(); return { code: `(i32.const ${value})`, value }; }
    const [op, run] = operators[(random()>>>0)%operators.length], a=expr(depth-1), b=expr(depth-1);
    return { code:`(${op} ${a.code} ${b.code})`, value:run(a.value,b.value) };
  }
  return Array.from({length:40}, () => expr(4));
}
for (const backend of ["core", "component"]) test(`${backend}: Walz parser and codegen compile and execute checked modules`, {timeout:180000}, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-selfhost-"));
  try {
    const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
    await build("tests/component-fixtures/selfhost.walz", directory); await instrument(directory, backend);
    const { instantiate } = await import(pathToFileURL(join(directory,"index.js")));
    async function create() {
      let arena;
      const runtime = await instantiate({ instantiateCore:async (m, imports) => {
        const value=await WebAssembly.instantiate(m,imports); if(value.exports.live) arena=value.exports; return value;
      } });
      return { api:runtime.api, clean:()=>{assert.equal(arena.live(),0);assert.ok(Object.values(runtime.__memory()).every(n=>n===0));} };
    }
    const { api, clean }=await create();
    assert.deepEqual([...api.buildBytes(0,new Uint8Array())],[]); clean();
    assert.deepEqual([...api.buildBytes(2,Uint8Array.of(0,255))],[0,255]); clean();
    for(const [limit,input] of [[0,Uint8Array.of(1)],[64,new Uint8Array(65)],[-1,new Uint8Array()],[16777217,new Uint8Array()]]) {
      const fresh=await create(); assert.throws(()=>fresh.api.buildBytes(limit,input)); fresh.clean();
    }
    async function compile(source) {
      const result=api.compile(bytes(source)); assert.equal(result.tag,"ok",JSON.stringify(result)); clean();
      assert.ok(WebAssembly.validate(result.val)); return (await WebAssembly.instantiate(result.val)).instance.exports;
    }
    // Token values are decoded bytes; spans always address the original UTF-8.
    const source='(; outer (; nested ;) ;) (module ;; line\n (func $f (export "🦀\\00") (result i32) 42))';
    const parsed=api.inspect(bytes(source)); assert.equal(parsed.tag,"ok",JSON.stringify(parsed));
    const root=parsed.val.nodes[parsed.val.roots[0]]; assert.equal(root.kind,2);
    assert.equal(root.start,source.indexOf('(module')); assert.equal(root.end,bytes(source).length);
    assert.equal(parsed.val.nodes[root.children[0]].kind,0);
    const string=parsed.val.nodes.find(n=>n.kind===1); assert.deepEqual([...string.value],[240,159,166,128,0]);
    const raw=String.raw`("\00\ff\u{1f980}\n\"" $"quoted id")`;
    const rawTree=api.inspect(bytes(raw)); assert.equal(rawTree.tag,"ok");
    assert.deepEqual([...rawTree.val.nodes.find(n=>n.kind===1).value],[0,255,240,159,166,128,10,34]);
    assert.ok(rawTree.val.nodes.some(n=>n.kind===3));
    assert.equal(api.inspect(bytes(' ;; empty\n')).val.roots.length,0); clean();
    for(const invalid of ['(',')','(;','"bad\\q"','"bad\nline"','"\\u{d800}"','"\\u{110000}"','"\\a"','"\\u{}"','"\\u{_1}"']) {
      const r=api.inspect(bytes(invalid)); assert.equal(r.tag,"err",invalid);
      assert.ok(r.val.start>=0&&r.val.end<=bytes(invalid).length); clean();
    }
    assert.equal(api.inspect(Uint8Array.of(0xff)).tag,"err");
    assert.equal(api.inspect(bytes('('.repeat(129)+')'.repeat(129))).tag,"err");
    assert.equal(api.inspect(new Uint8Array(1048577).fill(32)).tag,"err"); clean();
    assert.equal(api.compile(bytes('x '.repeat(65537))).val.code,8,"node budget is enforced before lowering"); clean();
    for(const [v,expected] of [[0,[0]],[127,[127]],[128,[128,1]],[624485,[229,142,38]],[-1,[255,255,255,255,15]]])
      assert.deepEqual([...api.uleb(v)],expected);
    for(const [v,expected] of [[0,[0]],[63,[63]],[64,[192,0]],[-1,[127]],[-64,[64]],[-65,[191,127]],[-2147483648,[128,128,128,128,120]],[2147483647,[255,255,255,255,7]]])
      assert.deepEqual([...api.sleb(v)],expected); clean();
    assert.equal((await compile('(module (func (export "answer") (result i32) (+ 40 2)))')).answer(),42);
    const functions=await compile(`(module
      (func $main (export "main") (result i32) (call $add 40 2))
      (func $add (param $a i32) (param $b i32) (result i32) (@effect pure) (+ $a $b))
      (func $fac (export "factorial") (param $n i32) (result i32)
        (if (result i32) (i32.le_s $n 1) (then 1) (else (* $n (call $fac (- $n 1))))))
      (export "addition" (func $add)))`);
    assert.equal(functions.main(),42); assert.equal(functions.addition(20,22),42); assert.equal(functions.factorial(6),720);
    const traps=await compile('(module (func (export "safe") (result i32) (if (result i32) 0 (then (i32.div_s 1 0)) (else 42))) (func (export "trap") (result i32) (i32.div_s 1 0)))');
    assert.equal(traps.safe(),42); assert.throws(()=>traps.trap(),WebAssembly.RuntimeError);
    const name='日本🦀'.repeat(30), wide=Array.from({length:130},(_,i)=>`(func $f${i} (result i32) ${i})`).join('\n');
    const wideExports=await compile(`(module ${wide} (func (export "${name}") (result i32) (call $f129)))`);
    assert.equal(wideExports[name](),129);
    const manyParams=await compile(`(module (func (export "last") ${'(param i32) '.repeat(130)} (result i32) (local.get 129)))`);
    assert.equal(manyParams.last(...Array.from({length:130},(_,i)=>i)),129);
    assert.equal((await compile('(module (func (export "x\\00\\u{1f980}") (result i32) 0xffffffff))'))['x\0🦀'](),-1);
    const generated=generatedCases(), text=`(module ${generated.map((c,i)=>`(func (export "f${i}") (result i32) ${c.code})`).join('\n')})`;
    const compiled=await compile(text);
    // Compile the exact input with the existing Rust/WAT path as a second oracle.
    const reference=join(directory,'reference.walz'), referenceWasm=join(directory,'reference.wasm'); await writeFile(reference,text);
    await exec('cargo',['run','--locked','--quiet','--','build',reference,referenceWasm]);
    const native=(await WebAssembly.instantiate(await readFile(referenceWasm))).instance.exports;
    for(const [i,c] of generated.entries()){assert.equal(compiled[`f${i}`](),c.value);assert.equal(compiled[`f${i}`](),native[`f${i}`]());}
    const cliOutput=join(directory,'answer.wasm');
    await exec(process.execPath,['runtime/selfhost.mjs','examples/selfhost/answer.walz',cliOutput,directory]);
    assert.equal((await WebAssembly.instantiate(await readFile(cliOutput))).instance.exports.main(),42);
    assert.match((await exec(process.execPath,['runtime/selfhost.mjs','--check','examples/selfhost/answer.walz',directory])).stdout,/Checked.*2 functions/);
    const invalidFile=join(directory,'invalid.walz'), invalidOutput=join(directory,'invalid.wasm');
    await writeFile(invalidFile,'(module (memory 1))');
    await assert.rejects(exec(process.execPath,['runtime/selfhost.mjs',invalidFile,invalidOutput,directory]),e=>e.code===1&&e.stderr.includes('error 101'));
    await assert.rejects(readFile(invalidOutput),{code:'ENOENT'});
    await writeFile(invalidFile,'(module (func (result i64) (i32.const 1)))');
    await assert.rejects(exec(process.execPath,['runtime/selfhost.mjs','--check',invalidFile,directory]),e=>e.code===1&&e.stderr.includes('error 110: type or stack-height mismatch'));
    for(const bad of [
      '(module (memory 1))','(module (func (result f64) 1))','(module (func (result i32) (call $missing)))',
      '(module (func (result i32) $missing))','(module (func $a (result i32) 0) (func $a (result i32) 1))',
      '(module (func (export "x") (export "x") (result i32) 0))','(module (func (result i32) 4294967296))',
      '(module (func (result i32) -2147483649))','(module (func (result i32) 0x100000000))',
      '(module (func (result i32) (i32.add 1)))','(module (func (result i32) (local.get 0)))',
      '(module (func $a (param $x i32) (result i32) $x) (func (result i32) (call $a)))',
      '(module (func (param $x i32) (param $x i32) (result i32) 0))',
      '(module (func (result i32) (if (result i32) 1 (then 2) (else (f64.const 3)))))',
      '(module (func (export "\\ff") (result i32) 0))', '(module) (module)',
      '(module (func (result i32) (call "0")))',
      '(module (func (result i32) 0) (export "x" (func "0")))',
      '(module (func $名前 (result i32) 0))',
    ]) {const r=api.compile(bytes(bad));assert.equal(r.tag,'err',bad);assert.ok(r.val.code>=100);assert.ok(r.val.start>=0&&r.val.end<=bytes(bad).length);clean();}
    assert.ok(WebAssembly.validate(api.compile(bytes('(module)')).val)); clean(); await collect(); clean();
    const unknown='(; 🦀 ;) (module (func (result i32) $missing))', at=bytes(unknown).indexOf(36);
    assert.deepEqual(api.compile(bytes(unknown)),{tag:'err',val:{code:109,start:at,end:at+8}}); clean();
    assert.equal(api.compile(bytes(`(module ${'(func (result i32) 0)'.repeat(1025)})`)).val.code,150); clean();
    assert.equal(api.compile(bytes(`(module (func ${'(param i32)'.repeat(257)} (result i32) 0))`)).val.code,150); clean();
    assert.equal(api.compile(bytes(`(module (func ${Array.from({length:1025},(_,i)=>`(export "x${i}")`).join(' ')} (result i32) 0))`)).val.code,150); clean();
  } finally { await rm(directory,{recursive:true,force:true}); }
});
