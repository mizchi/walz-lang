import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm, copyFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createWasiFileSystem } from './wasi-filesystem.mjs';
import { createNodeFileSystem } from './node-filesystem.mjs';
import { execFile as execFileCallback } from 'node:child_process';
import { promisify } from 'node:util';
import { pathToFileURL } from 'node:url';
const execFile=promisify(execFileCallback);

test('file completion waits for descriptor disposal on cancellation',async()=>{
  const closed=Promise.withResolvers();let disposed=false,settled=false;
  const files=createWasiFileSystem({readFiles:{input:{path:'x',directory:{async openAt(){return {
    readViaStream:()=>[(async function*(){yield 1;})(),Promise.resolve({tag:'ok',val:undefined})],
    async [Symbol.asyncDispose](){await closed.promise;disposed=true;},
  };}}}}});
  const [stream,complete]=files.readFile('input');await stream.next();
  complete.then(()=>{settled=true;});const closing=stream.return();
  await Promise.resolve();await Promise.resolve();assert.equal(settled,false);
  closed.resolve();await closing;await complete;assert.equal(disposed,true);assert.equal(files.active(),0);
});

test('read cleanup attempts iterator return even if descriptor close fails',async()=>{
  let returned=0;
  const iterator={next:async()=>({done:false,value:1}),return:async()=>{returned++;return {done:true};},[Symbol.asyncIterator](){return this;}};
  const files=createWasiFileSystem({readFiles:{input:{path:'x',directory:{async openAt(){return {
    readViaStream:()=>[iterator,Promise.resolve({tag:'ok',val:undefined})],
    [Symbol.asyncDispose](){throw new Error('close failed');},
  };}}}}});
  const [stream,complete]=files.readFile('input');await stream.next();
  await assert.rejects(stream.return(),/close failed|cleanup/);await complete;
  assert.equal(returned,1);assert.equal(files.active(),0);
});

test('write cleanup attempts input return even if descriptor close fails',async()=>{
  let returned=0;
  const iterator={next:async()=>({done:true}),return:async()=>{returned++;return {done:true};},[Symbol.asyncIterator](){return this;}};
  const files=createWasiFileSystem({writeFiles:{output:{path:'x',directory:{async openAt(){return {
    writeViaStream:async()=>({tag:'ok',val:undefined}),
    [Symbol.asyncDispose](){throw new Error('close failed');},
  };}}}}});
  await assert.rejects(files.writeFile('output',iterator),/close failed|cleanup/);
  assert.equal(returned,1);assert.equal(files.active(),0);
});

test('WASI 0.3 descriptors stay host-owned and close after read completion or cancellation', async () => {
  let opened=0,closed=0;
  const directory={async openAt(flags,path,open,access) {
    assert.equal(path,'selected.bin'); assert.equal(access.read,true); opened++;
    return {readViaStream(offset){assert.equal(offset,0n);return [(async function*(){yield 0;yield 255;})(),Promise.resolve({tag:'ok',val:undefined})];},async [Symbol.asyncDispose](){closed++;}};
  }};
  const files=createWasiFileSystem({readFiles:{input:{directory,path:'selected.bin'}}});
  const [stream,completion]=files.readFile('input');
  assert.deepEqual(await Array.fromAsync(stream),[0,255]);
  assert.deepEqual(await completion,{tag:'ok',val:undefined});
  assert.equal(opened,1);assert.equal(closed,1);
  const [cancelled,result]=files.readFile('input');
  await cancelled.next();await cancelled.return();
  assert.deepEqual(await result,{tag:'err',val:'interrupted'});
  assert.equal(closed,2);
  const [denied,denial]=files.readFile('other');
  assert.deepEqual(await Array.fromAsync(denied),[]);
  assert.deepEqual(await denial,{tag:'err',val:'not-permitted'});
  assert.equal(opened,2);
  await files.close(); assert.equal(closed,2);
});

test('Node file grants use the same stream/completion protocol and release handles', async () => {
  const directory=await mkdtemp(join(tmpdir(),'wlt-wasi-files-'));
  try {
    const input=join(directory,'in.bin'),output=join(directory,'out.bin');
    const bytes=new Uint8Array([0,1,127,128,255]);await writeFile(input,bytes);
    const files=createNodeFileSystem({readFiles:{input},writeFiles:{output}});
    const [stream,complete]=files.readFile('input');
    const written=files.writeFile('output',stream);
    assert.deepEqual(await complete,{tag:'ok',val:undefined});
    assert.deepEqual(await written,{tag:'ok',val:undefined});
    assert.deepEqual(new Uint8Array(await readFile(output)),bytes);
    const [missing,error]=files.readFile('missing');
    assert.deepEqual(await Array.fromAsync(missing),[]);assert.equal((await error).val,'not-permitted');
    await files.close();assert.equal(files.active(),0);
  } finally {await rm(directory,{recursive:true,force:true});}
});

test('file batches transfer whole chunks and dispose a failed batch provider',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'wlt-file-batches-'));
  try{
    const input=join(dir,'input'),output=join(dir,'output'),bytes=Uint8Array.from({length:8193},(_,i)=>i&255);
    await writeFile(input,bytes);const files=createNodeFileSystem({readFiles:{input},writeFiles:{output}});
    const[stream,complete]=files.readFile('input');let batches=0;const read=stream.readMany.bind(stream);stream.readMany=max=>{batches++;return read(max);};
    assert.equal((await files.writeFile('output',stream)).tag,'ok');assert.equal((await complete).tag,'ok');
    assert.equal(batches,2);assert.deepEqual(new Uint8Array(await readFile(output)),bytes);await files.close();assert.equal(files.active(),0);
    let returned=0,closed=0;
    const broken=createWasiFileSystem({readFiles:{input:{path:'chosen',directory:{async openAt(){return {readViaStream:()=>[{[Symbol.asyncIterator](){return this;},readMany:async()=>{throw 'io';},return(){returned++;return {done:true};}},Promise.resolve({tag:'ok',val:undefined})],async [Symbol.asyncDispose](){closed++;}};}}}}});
    const[failed,result]=broken.readFile('input');assert.deepEqual(await failed.readMany(64),{items:[],done:true});assert.equal((await result).tag,'err');assert.equal(returned,1);assert.equal(closed,1);assert.equal(broken.active(),0);
  }finally{await rm(dir,{recursive:true,force:true});}
});

test('read completion waits for descriptor disposal and unused cancellation does not open a file', async () => {
  const gate=Promise.withResolvers();let opened=0,settled=false;
  const directory={async openAt(){opened++;return {
    readViaStream(){return [(async function*(){})(),Promise.resolve({tag:'ok',val:undefined})];},
    async [Symbol.asyncDispose](){await gate.promise;},
  };}};
  const files=createWasiFileSystem({readFiles:{input:{directory,path:'input'}}});
  const [unused,future]=files.readFile('input');await unused.return();assert.equal((await future).val,'interrupted');assert.equal(opened,0);
  const [stream,completion]=files.readFile('input');completion.then(()=>{settled=true;});
  const read=stream.next();await new Promise(resolve=>setImmediate(resolve));assert.equal(settled,false);
  gate.resolve();assert.deepEqual(await read,{done:true});assert.equal((await completion).tag,'ok');
  await files.close();assert.equal(files.active(),0);
});

for(const backend of ['core','component']) test(`${backend}: file adapter transports byte streams and completion results without retaining descriptors`,{timeout:60000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'wlt-file-abi-'));
  try {
    for(const name of ['main.wlt','batch.wlt','filesystem.wit','policy.json'])await copyFile(`examples/wasi-files/${name}`,join(directory,name));
    await execFile('cargo',['run','--locked','--quiet','--','bind',join(directory,'filesystem.wit'),join(directory,'policy.json'),join(directory,'generated')]);
    const {build}=await import(backend==='core'?'./build-core.mjs':'./build.mjs');
    const out=join(directory,'out');await build(join(directory,'main.wlt'),out,{bindings:join(directory,'generated/bindings.json')});
    const input=join(directory,'input'),output=join(directory,'output');await writeFile(input,new Uint8Array([65,66]));
    const files=createNodeFileSystem({readFiles:{input},writeFiles:{output}});
    const {instantiate}=await import(pathToFileURL(join(out,'index.js')));let allocator;
    const {api}=await instantiate({imports:{'wlt:filesystem/files@0.3.0':files},instantiateCore:async(m,i)=>{const instance=await WebAssembly.instantiate(m,i);if(instance.exports.live)allocator=instance.exports;return instance;}});
    assert.equal(await api.main(),131);assert.equal(await api.write(),0);
    assert.equal(await api.sumBatch(1),131);assert.equal(await api.sumBatch(256),131);
    assert.deepEqual(new Uint8Array(await readFile(output)),new Uint8Array([65,66]));
    await rm(input);assert.equal(await api.main(),-1);
    await files.close();assert.equal(files.active(),0);assert.equal(allocator.live(),0);
  } finally {await rm(directory,{recursive:true,force:true});}
});
