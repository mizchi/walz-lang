import {test,expect} from '@playwright/test';
test('staged record constructors preserve future and stream payloads in Chromium',async({page})=>{
  await page.goto('/');
  const out=await page.evaluate(async()=>{
    const {instantiate}=await import('/typed-constructors/index.js'),memories=[];
    const {api}=await instantiate({imports:{'test:constructors/host':{}},instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;}});
    const item={value:4294967295,label:'日本😀'.repeat(4096)};
    const echo=api.first.echo(item),again=api.again.echo(item),relay=api.relay.echo(item);
    const ready=await(await api.first.ready(item)),future=await(await api.future.transfer(Promise.resolve(item)));
    const consumed=await api.await.consume(Promise.resolve(item)),stream=[];
    for await(const v of await api.stream.transfer((async function*(){yield item;yield item;})()))stream.push(v);
    await new Promise(r=>setTimeout(r,25));
    return {echo,again,relay,ready,future,consumed,stream,live:memories.length>0&&memories.every(m=>m.live()===0)};
  });
  const item={value:4294967295,label:'日本😀'.repeat(4096)};
  expect(out).toEqual({echo:item,again:item,relay:item,ready:item,future:item,consumed:item,stream:[item,item],live:true});
});
test('nominal value ascriptions preserve shared resource identity in Chromium',async({page})=>{
  await page.goto('/');
  const out=await page.evaluate(async()=>{
    const {instantiate}=await import('/nominal-resources/index.js');let closed=0;
    class File {constructor(){this.closed=false;}[Symbol.dispose](){if(this.closed)throw new Error('double close');this.closed=true;closed++;}}
    const {api}=await instantiate({imports:{'test:resources/host':{File,make:()=>new File(),read:f=>{if(f.closed)throw new Error('closed');return 42;}}}});
    const file=await api.left.make(),read=await api.right.read(file);
    await api.right.close(file);const handled=await api.left.handled();
    return {read,handled,closed,value:api.left.echo({value:42})};
  });
  expect(out).toEqual({read:42,handled:42,closed:2,value:{value:42}});
});
test('connected type queries preserve record, unsigned and async values in Chromium',async({page})=>{
  await page.goto('/');
  const out=await page.evaluate(async()=>{
    const {instantiate}=await import('/queries/index.js');const memories=[];
    const host={echo:v=>v,number:v=>v,itemValue:v=>v.value,copyItem:async v=>v,readyItem:v=>Promise.resolve(v),consume:async v=>await v};
    const {api}=await instantiate({imports:{'test:queries/host':host},instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;}});
    const item={value:42,label:'日本😀'.repeat(4096)},input=Int32Array.from({length:65536},(_,i)=>i-42);
    const values=api.echo(input),record=api.echoItem(item),future=await (await api.readyItem(item)),text=await api.consume(Promise.resolve(item.label));
    const copied=await api.copyItem(item);
    await new Promise(r=>setTimeout(r,25));
    return {number:api.number(4294967295),value:api.read(item),record,copied,future,text,values:values.length===input.length&&values.every((v,i)=>v===input[i]),live:memories.length>0&&memories.every(m=>m.live()===0)};
  });
  expect(out).toEqual({number:4294967295,value:42,record:{value:42,label:'日本😀'.repeat(4096)},copied:{value:42,label:'日本😀'.repeat(4096)},future:{value:42,label:'日本😀'.repeat(4096)},text:'日本😀'.repeat(4096),values:true,live:true});
});
test('generic constructors specialize portable values and retain per-instance state in Chromium',async({page})=>{
  await page.goto('/');
  const out=await page.evaluate(async()=>{
    const {instantiate}=await import('/generics/index.js');const memories=[];
    const {api}=await instantiate({instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;}});
    const text='日本😀'.repeat(4096),values=Int32Array.from({length:65536},(_,i)=>i-42);
    const echoed=api.values.echo(values);
    const counters=[api.narrow.next(),api.narrow.next(),api.again.next(),(await instantiate()).api.narrow.next()];
    const future=await (await api.pending.transfer(Promise.resolve(42)));
    await new Promise(r=>setTimeout(r,25));
    return {value:api.narrow.echo(42),wide:String(api.wide.echo(9007199254740993n)),text:api.text.echo(text)===text,
      values:echoed.length===values.length&&echoed.every((v,i)=>v===values[i]),none:api.values.echo(undefined)===undefined,
      counters,future,buffersRetired:memories.length>0&&memories.every(m=>m.live()===0)};
  });
  expect(out).toEqual({value:42,wide:'9007199254740993',text:true,values:true,none:true,counters:[1,2,1,1],future:42,buffersRetired:true});
});
test('native composition runs generic APIs and independent state in Chromium',async({page})=>{
  await page.goto('/');
  const out=await page.evaluate(async()=>{
    const {instantiate}=await import('/index.js');
    const {api}=await instantiate();
    return [api.add(20,22),String(api.identity(42n)),await api.run(),await api.run(),await api.otherCounter.next(),await (await instantiate()).api.run()];
  });
  expect(out).toEqual([42,'42',41,42,1,41]);
});
test('inferred resources preserve identity, borrows and scoped exception cleanup in Chromium',async({page})=>{
  await page.goto('/');
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import('/resources/index.js');let closed=0;const memories=[];
    class File {constructor(){this.value=42;this.closed=false;}[Symbol.dispose](){if(this.closed)throw new Error('double close');this.closed=true;closed++;}}
    const host={File,make:()=>new File(),read:f=>{if(f.closed)throw new Error('read after close');return f.value;}};
    const {api}=await instantiate({imports:{'test:resources/host':host},instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memories.push(x.exports);return x;}});
    const file=await api.make();const value=await api.read(file);const borrowedClosed=closed;
    const same=await api.echo(file)===file;await api.close(file);
    const optional=await api.make();const wrapped=await api.optional(optional)===optional;
    await api.releaseOption(optional);await api.releaseOption(undefined);
    const handled=await api.handled();
    return {value,borrowedClosed,same,wrapped,handled,closed,live:memories.map(m=>m.live())};
  });
  expect(result).toEqual({value:42,borrowedClosed:0,same:true,wrapped:true,handled:42,closed:3,live:[0]});
});
test('private resource providers retire owners and isolate constructor state in Chromium',async({page})=>{
  await page.goto('/');
  const out=await page.evaluate(async()=>{
    const {instantiate}=await import('/private-resources/index.js');let memory;
    const {api}=await instantiate({instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memory=x.exports;return x;}});
    for(let i=0;i<20;i++){if(await api.run()!==42)throw new Error('value');if(memory.live()!==0)throw new Error('ABI buffer');}
    const other=await instantiate();
    return {closed:await api.closedCount(),fresh:await other.api.closedCount(),live:memory.live()};
  });
  expect(out).toEqual({closed:20,fresh:0,live:0});
});
test('native composition frees value and async transfer buffers in Chromium',async({page})=>{
  await page.goto('/');
  const out=await page.evaluate(async()=>{
    const {instantiate}=await import('/index.js');const memories=[];
    const {api}=await instantiate({instantiateCore:async(m,imports)=>{
      const x=await WebAssembly.instantiate(m,imports);if(x.exports.live)memories.push(x.exports);return x;
    }});
    const value='日本😀'.repeat(4096),input=Uint8Array.from({length:65536},(_,i)=>i);
    const output=api.echo(input);
    const text=await(await api.readyText(value));
    const answer=await api.consume(Promise.resolve(42));
    const stream=await api.prepend((async function*(){yield 8;yield 9;})());
    const items=[];for await(const batch of stream)items.push(...batch);
    await new Promise(r=>setTimeout(r,25));
    const live=memories.map(m=>m.live());
    const failed=await instantiate();
    let rejected=false;try{await failed.api.consume(Promise.reject(new Error('host failure')));}catch{rejected=true;}
    return {text:text===value,bytes:output.every((v,i)=>v===input[i])&&output.length===input.length,answer,items,rejected,live};
  });
  expect(out).toEqual({text:true,bytes:true,answer:42,items:[7,8,9],rejected:true,live:[0,0]});
});
