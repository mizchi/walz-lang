import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {build} from './build.mjs';

test('queried types preserve unsigned, record and endpoint ABIs without retaining transfer buffers',{timeout:120000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-component-queries-'));
  try {
    await build('examples/components/type-queries.walz',dir);
    const {instantiate}=await import(pathToFileURL(join(dir,'index.js')));
    const host={echo:v=>v,number:v=>v,itemValue:v=>v.value,copyItem:async v=>{await new Promise(r=>setTimeout(r,1));return v;},readyItem:v=>Promise.resolve(v),consume:async v=>await v};
    const memories=[];
    const options={imports:{'test:queries/host':host},instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;}};
    const {api}=await instantiate(options);
    assert.equal(api.number(4294967295),4294967295);
    assert.equal(api.field(42),42);
    for(let n=0;n<15;n++) {
      const values=Int32Array.from({length:n*4096},(_,i)=>i-42),item={value:42,label:'日本😀'.repeat(n*1024)};
      assert.deepEqual(api.echo(values),values);assert.equal(api.echo(undefined),undefined);
      assert.equal(api.read(item),42);assert.deepEqual(api.echoItem(item),item);
      assert.deepEqual(await api.copyItem(item),item);
      assert.deepEqual(await (await api.readyItem(item)),item);
      assert.equal(await api.consume(new Promise(r=>setTimeout(()=>r(item.label),1))),item.label);
    }
    await new Promise(r=>setTimeout(r,25));
    assert.ok(memories.length>0);
    for(const m of memories)assert.equal(m.live(),0,'retained canonical buffer');
    const failed=await instantiate(options);
    await assert.rejects(failed.api.consume(Promise.reject(new Error('host failure'))));
    assert.equal(await (await instantiate(options)).api.consume(Promise.resolve('fresh')),'fresh');
  } finally {await rm(dir,{recursive:true,force:true});}
});

test('queried resource signatures retain explicit moves and scoped destruction',{timeout:60000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'walz-resource-queries-'));
  try {
    const original=await readFile('examples/components/resources.walz','utf8');
    const source=original.replaceAll('(result (@resource $File))','(result (@component.type "test:resources/host" (result "make")))')
      .replaceAll('(param $f $borrow<$File>)','(param $f (@component.type "test:resources/host" (param "read" 0)))');
    const input=join(dir,'resources.walz');await writeFile(input,source);await build(input,join(dir,'out'));
    const {instantiate}=await import(pathToFileURL(join(dir,'out','index.js')));let closed=0;
    class File {constructor(){this.value=42;this.closed=false;}[Symbol.dispose](){assert.equal(this.closed,false);this.closed=true;closed++;}}
    const {api}=await instantiate({imports:{'test:resources/host':{File,make:()=>new File(),read:f=>{assert.equal(f.closed,false);return f.value;}}}});
    for(let n=0;n<20;n++) {
      const f=await api.make();assert.equal(await api.read(f),42);assert.equal(closed,n*2);
      assert.equal(await api.echo(f),f);await api.close(f);
      assert.equal(await api.handled(),42);assert.equal(closed,(n+1)*2);
    }
  } finally {await rm(dir,{recursive:true,force:true});}
});
