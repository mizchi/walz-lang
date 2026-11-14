import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {instrument,collect} from './testing/memory.mjs';
const aliases=[['=',70,81],['!=',71,82],['<',72,83],['>',74,85],['<=',76,87],['>=',78,89],
  ['+',106,124],['-',107,125],['*',108,126],['/',109,127],['&',113,131],['|',114,132],['^',115,133],
  ['<<',116,134],['>>',117,135],['>>>',118,136]];
const map=new Map(aliases.map(([name,...codes])=>[Buffer.from(name).toString('hex'),codes]));
const spellings=[new Uint8Array(),...aliases.map(([name])=>new TextEncoder().encode(name)),
  ...['==','===','<<=','>>=','!=x','>>>x','< >',' +','+\0','&&','||','λ'].map(s=>new TextEncoder().encode(s))];
for(let byte=0;byte<256;byte++)spellings.push(Uint8Array.of(byte),Uint8Array.of(byte,61),Uint8Array.of(62,62,byte));
for(const backend of ['core','component'])test(`${backend}: scalar alias lookup preserves exact spellings and widths`,{timeout:60000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-selfhost-operators-'));
  try{
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    await build('tests/component-fixtures/selfhost-operators.walz',directory);await instrument(directory,backend);
    let arena;
    const value=await(await import(pathToFileURL(join(directory,'index.js')))).instantiate({instantiateCore:async(m,imports)=>{
      const instance=await WebAssembly.instantiate(m,imports);if(instance.exports.live)arena=instance.exports;return instance;
    }});
    for(const spelling of spellings){
      const key=Buffer.from(spelling).toString('hex'),codes=map.get(key);
      for(const width of [127,126,64,-1])assert.equal(value.api.alias(spelling,width),codes?.[+(width===126)]??0,key+' '+width);
      assert.equal(value.api.comparison(spelling),+(codes!==undefined&&codes[0]<100),key+' comparison');
    }
    for(const c of [-1,...Array.from({length:256},(_,i)=>i),256,2147483647,-2147483648]){
      const expected=c>=48&&c<=57?c-48:(c|32)>=97&&(c|32)<=102?(c|32)-87:-1;
      assert.equal(value.api.hex(c),expected,'hex digit '+c);
    }
    await collect();assert.equal(arena.live(),0);assert.ok(Object.values(value.__memory()).every(n=>n===0));
  }finally{await rm(directory,{recursive:true,force:true});}
});
