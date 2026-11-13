import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { instrument, collect } from "./testing/memory.mjs";
const bytes=s=>new TextEncoder().encode(s), exec=promisify(callback);
const instruction=(op,immediate=0)=>({op,immediate:BigInt(immediate),start:0,end:0});
const validPrograms=[
  ['(module (func (export "f")))',[],undefined],
  ['(module (func (export "f") (result i64) 18446744073709551615))',[],-1n],
  ['(module (func (export "f") (result i64) -9223372036854775808))',[],-9223372036854775808n],
  ['(module (func (export "f") (param $a i32) (param $b i64) (result i64) (+ (i64.extend_i32_s $a) $b)))',[40,2n],42n],
  ['(module (func (export "f") (param $n i64) (result i32) (= $n 42)))',[42n],1],
  ['(module (func (export "f") (result i32) (i32.wrap_i64 (i64.const 0x10000002a))))',[],42],
  ['(module (func $u) (func (export "f") (result i32) (call $u) 42))',[],42],
  ['(module (func (export "f") (param $n i64) (result i64) (local $acc i64) (block $done (loop $again (br_if $done (i64.eqz $n)) (local.set $acc (+ $acc $n)) (local.set $n (- $n 1)) (br $again))) $acc))',[100n],5050n],
  ['(module (func (export "f") (param $flag i32) (result i64) (block $out (result i64) (drop (br_if $out 42 $flag)) 7)))',[1],42n],
  ['(module (func (export "f") (param $flag i32) (result i64) (block $out (result i64) (drop (br_if $out 42 $flag)) 7)))',[0],7n],
  ['(module (func (export "f") (result i64) (block (result i64) (return 42))))',[],42n],
  ['(module (func (export "f") (result i64) (@let $x (type i64) 40 (+ $x 2))))',[],42n],
  ['(module (func (export "f") (result i32) (@let $x (i32.const 40) (@let $x (+ $x 2) $x))))',[],42],
  ['(module (func (export "f") (result i32) (local $x i32) (local.tee $x 42)))',[],42],
  ['(module (func (export "f") (result i64) (if (result i64) 0 (then (unreachable)) (else 42))))',[],42n],
  ['(module (func (export "f") (result i64) (select (i64.const 42) (i64.const 7) 1)))',[],42n],
];
for(const backend of ['core','component'])test(`${backend}: independent checker validates typed control and mutable locals`,{timeout:180000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-checker-'));
  try {
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/selfhost.walz',directory);await instrument(directory,backend);
    const {instantiate}=await import(pathToFileURL(join(directory,'index.js')));let arena;
    const value=await instantiate({instantiateCore:async(m,imports)=>{const v=await WebAssembly.instantiate(m,imports);if(v.exports.live)arena=v.exports;return v;}});
    const {api}=value,clean=()=>{assert.equal(arena.live(),0);assert.ok(Object.values(value.__memory()).every(n=>n===0));};
    assert.equal(typeof api.check,'function');assert.equal(typeof api.checkIr,'function');
    for(const [source,args,expected] of validPrograms){
      const checked=api.check(bytes(source));assert.equal(checked.tag,'ok',source+' '+JSON.stringify(checked));clean();
      const r=api.compile(bytes(source));assert.equal(r.tag,'ok',source+' '+JSON.stringify(r));assert.ok(WebAssembly.validate(r.val));clean();
      const generated=(await WebAssembly.instantiate(r.val)).instance.exports;
      assert.equal(generated.f(...args),expected,source);
    }
    // Exact standard-WAT inputs use the pinned wat/wasmparser crates as an
    // independent oracle, without the existing frontend's folded-body limits.
    const watPrograms=[
      ['(module (func (export "f")))',[],undefined],
      ['(module (func (export "f") (result i64) (i64.const 0xffffffffffffffff)))',[],-1n],
      ['(module (func (export "f") (result i64) (i64.const -9223372036854775808)))',[],-9223372036854775808n],
      ['(module (func (export "f") (param $n i64) (result i64) (local $acc i64) (block $done (loop $again (br_if $done (i64.eqz (local.get $n))) (local.set $acc (i64.add (local.get $acc) (local.get $n))) (local.set $n (i64.sub (local.get $n) (i64.const 1))) (br $again))) (local.get $acc)))',[100n],5050n],
      ['(module (func (export "f") (result i64) (block (result i64) (return (i64.const 42)))))',[],42n],
      ['(module (func (export "f") (result i32) (unreachable) (drop (unreachable))))',[],null],
    ];
    for(const [source,args,expected]of watPrograms){
      const input=join(directory,'reference.wat'),wasm=join(directory,'reference.wasm');await writeFile(input,source);
      await exec('cargo',['run','--locked','--quiet','--example','selfhost-reference','--',input,wasm]);
      const r=api.compile(bytes(source));assert.equal(r.tag,'ok',source+' '+JSON.stringify(r));assert.ok(WebAssembly.validate(r.val));clean();
      const ours=(await WebAssembly.instantiate(r.val)).instance.exports,reference=(await WebAssembly.instantiate(await readFile(wasm))).instance.exports;
      if(expected===null){assert.throws(()=>ours.f(...args),WebAssembly.RuntimeError);assert.throws(()=>reference.f(...args),WebAssembly.RuntimeError);}
      else{assert.equal(ours.f(...args),expected);assert.equal(ours.f(...args),reference.f(...args));}
    }
    for(const source of [
      '(module (func (result i64) (i32.const 1)))',
      '(module (func (result i64) (i64.add (i32.const 1) (i64.const 2))))',
      '(module (func (param $x i64) (if $x (then (nop)))))',
      '(module (func (local $x i64) (local.set $x (i32.const 1))))',
      '(module (func (result i64) (if (result i64) 1 (then (i32.const 1)) (else 2))))',
      '(module (func (result i64) (block $b (result i64) (br $b (i32.const 1)))))',
      '(module (func (br $missing)))',
      '(module (func (local $x i32) (local $x i64)))',
      '(module (func (result i64) (@let $x (type i64) 1 (local.tee $x 2))))',
      '(module (func (result i32) (block (@let $x (i32.const 1) (drop $x))) $x))',
      '(module (func (result i32) (@let $x (type i32) 1 (local.get 0))))',
      '(module (func (result i32) (< 1 2)))',
      '(module (func (result i64) 18446744073709551616))',
      '(module (func (result i64) -9223372036854775809))',
      '(module (func (result i64) (i64.add (unreachable) (i32.const 1))))',
      '(module (func (result i64) (i32.add (unreachable) (i32.const 1))))',
      '(module (func (result i32) ("i32.add" 1 2)))',
    ]){const r=api.check(bytes(source));assert.equal(r.tag,'err',source);assert.ok(r.val.code>=100);assert.equal(api.compile(bytes(source)).tag,'err');clean();}
    const rawCases=[
      [[],[],64,[],true],
      [[],[],126,[instruction(66,42)],true],
      [[],[],126,[instruction(65,42)],false],
      [[],[],126,[instruction(0),instruction(26)],true],
      [[],[],126,[instruction(0),instruction(65,1)],false],
      [[],[],126,[instruction(0),instruction(65,1),instruction(106)],false],
      [[],[],127,[instruction(65,1),instruction(26)],false],
      [[],[],64,[instruction(65,1)],false],
      [[],[],64,[instruction(255)],false],
      [[],[],127,[instruction(65,2147483648n)],false],
      [[],[],64,[instruction(11)],false],
      [[],[],64,[instruction(5)],false],
      [[],[],64,[instruction(2,64)],false],
      [[],[],64,[instruction(2,126),instruction(65,1),instruction(12,0),instruction(11)],false],
      [[],[],64,[instruction(2,64),instruction(12,2),instruction(11)],false],
      [[],[],126,[instruction(65,1),instruction(4,126),instruction(66,42),instruction(11)],false],
      [[],[],127,[instruction(65,1),instruction(4,127),instruction(65,42),instruction(5),instruction(65,7),instruction(11)],true],
      [[],[126],126,[instruction(66,42),instruction(34,0)],true],
      [[],[126],64,[instruction(65,42),instruction(33,0)],false],
      [[127],[],126,[instruction(66,42),instruction(16,0)],false],
      [[],[],64,[instruction(16,1)],false],
      [[],[],64,[instruction(16,-1)],false],
      [[],[],64,[instruction(12,4294967296n)],false],
      [[],[],64,[instruction(1,1)],false],
      [[99],[],64,[],false],
      [[],[99],64,[],false],
      [[],Array(1025).fill(127),64,[],false],
    ];
    for(const [params,locals,result,code,expected]of rawCases){const r=api.checkIr(params,locals,result,code);assert.equal(r.tag,expected?'ok':'err',JSON.stringify({params,locals,result,code},(_,v)=>typeof v==='bigint'?String(v):v));clean();}
    // Source ranges and the control-depth budget are additional IR contracts,
    // independent of binary validation. Errors must leave the instance usable.
    for(const [start,end]of [[-1,0],[2,1],[0,1048577]]){
      const r=api.checkIr([],[],64,[{...instruction(1),start,end}]);
      assert.equal(r.tag,'err');assert.equal(r.val.code,117);clean();
    }
    for(const [depth,expected]of [[128,'ok'],[129,'err']]){
      const code=[...Array.from({length:depth},()=>instruction(2,64)),...Array.from({length:depth},()=>instruction(11))];
      const r=api.checkIr([],[],64,code);assert.equal(r.tag,expected);
      if(expected==='err')assert.equal(r.val.code,150);clean();
    }
    const corpus=rawCases.map(([params,locals,result,code])=>({params,locals,result,code}));
    let seed=0x794f421;
    const random=()=>{seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return seed>>>0;};
    for(let i=0;i<96;i++){
      const t=i&1?126:127,op=t===126?66:65;
      const code=[instruction(65,i&1),instruction(4,t),instruction(op,i),instruction(5),instruction(op,-i),instruction(11)];
      corpus.push({params:[],locals:[],result:t,code});
      corpus.push({params:[],locals:[],result:t,code:code.filter((_,j)=>j!==random()%code.length)});
    }
    const pool=[0,1,2,3,4,5,11,12,13,15,16,26,27,32,33,34,65,66,69,80,106,107,124,125,167,172,173];
    for(let i=0;i<384;i++){
      const code=Array.from({length:random()%18},()=>{
        const op=pool[random()%pool.length];let imm=0;
        if([2,3,4].includes(op))imm=[64,126,127][random()%3];
        if([12,13,16,32,33,34].includes(op))imm=random()%4;
        if([65,66].includes(op))imm=(random()|0);
        return instruction(op,imm);
      });
      corpus.push({params:random()&1?[127]:[],locals:random()&1?[126]:[],result:[64,126,127][random()%3],code});
    }
    const input=join(directory,'ir.json'),report=join(directory,'ir-results.json');
    await writeFile(input,JSON.stringify(corpus,(_,v)=>typeof v==='bigint'?String(v):v));
    await exec('cargo',['run','--locked','--quiet','--example','selfhost-reference','--','--ir',input,report]);
    const reference=JSON.parse(await readFile(report,'utf8'));assert.equal(reference.length,corpus.length);
    assert.ok(reference.filter(Boolean).length>90,'the differential corpus includes valid programs');
    for(const [i,raw]of corpus.entries()){
      const r=api.checkIr(raw.params,raw.locals,raw.result,raw.code);
      assert.equal(r.tag==='ok',reference[i],`IR differential ${i}: ${JSON.stringify(raw,(_,v)=>typeof v==='bigint'?String(v):v)}`);clean();
    }
    await collect();clean();
  }finally{await rm(directory,{recursive:true,force:true});}
});
