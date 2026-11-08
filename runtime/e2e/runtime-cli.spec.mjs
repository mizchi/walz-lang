import {test,expect} from '@playwright/test';

for(const backend of ['core','component'])test(`${backend}: portable types, validated WAT, channels and scoped tasks`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const load=name=>import(new URL(name+'/index.js',location.href).href);
    const {instantiate:types}=await load('interface-types');
    const {api:t}=await types();
    const item={name:'日本語',count:4294967295,color:'blue'};
    const record=t.item(item),wide=String(t.wide(18446744073709551615n));
    const {instantiate:memory}=await load('wasm-bridge');const {api:m}=await memory();
    const scalar=m.main(),simd=m.simd();
    const {instantiate:tasks}=await load('scoped-tasks');const {api:s}=await tasks();
    const task=await s.main();
    const {createChannels}=await import('/channels.mjs');const channels=createChannels();
    const {instantiate,TaskCancelledError}=await load('channels');let allocated,pending;
    const instance=await instantiate({imports:{channels:{...channels,streamReader:channels.reader,futureReader:channels.reader,closeStream:channels.close,closeFuture:channels.close},consumer:{start:async()=>{}}},
      onTaskEvent:e=>{if(e.type==='host-pending')pending?.resolve();},
      instantiateCore:async(module,imports)=>{const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)allocated=instance.exports;return instance;},
    });
    const pipeline=await instance.api.pipeline(),future=await instance.api.resolved();
    pending=Promise.withResolvers();const waiting=instance.start('pending');await pending.promise;
    await waiting.cancel();let cancelled=false;try{await waiting.result;}catch(error){cancelled=error instanceof TaskCancelledError;}
    const active=channels.active(),live=allocated.live();
    return {record,wide,scalar,simd,task,pipeline,future,cancelled,active,live};
  });
  expect(result).toEqual({record:{name:'日本語',count:4294967295,color:'blue'},wide:'18446744073709551615',scalar:42,simd:30,task:42,pipeline:42,future:{tag:'ok',val:42},cancelled:true,active:0,live:0});
});

for(const backend of ['core','component'])test(`${backend}: WASI descriptor adapter keeps file authority in the host`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {createWasiFileSystem,FILESYSTEM}=await import('/wasi-filesystem.mjs');
    let opened=0,closed=0;const output=[];
    const directory={async openAt(flags,path,open,access){opened++;return {
      readViaStream:()=>[(async function*(){yield 65;yield 66;})(),Promise.resolve({tag:'ok',val:undefined})],
      async writeViaStream(items){for await(const byte of items)output.push(byte);return {tag:'ok',val:undefined};},
      async [Symbol.asyncDispose](){closed++;},
    };}};
    const files=createWasiFileSystem({readFiles:{input:{directory,path:'chosen-input'}},writeFiles:{output:{directory,path:'chosen-output'}}});
    const {instantiate}=await import(new URL('wasi-files/index.js',location.href).href);
    const {api}=await instantiate({imports:{[FILESYSTEM]:files}});
    const sum=await api.main(),written=await api.write();await files.close();
    return {sum,written,output,opened,closed,active:files.active()};
  });
  expect(result).toEqual({sum:131,written:0,output:[65,66],opened:2,closed:2,active:0});
});
