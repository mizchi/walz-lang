import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: browser generic list values and failed conversions release their buffers`,async({page})=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import(new URL('lists/index.js',location.href).href);let memory;
    const {api}=await instantiate({instantiateCore:async(module,imports)=>{const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)memory=instance.exports;return instance;}});
    const literal=Array.from(api.literal(),String),snapshot=Array.from(api.snapshot(),String);
    const texts=api.texts(['','日本語🦀']),matrix=api.matrix([[1,2],[],[3]]).map(v=>Array.from(v));
    const future=await api.await(Promise.resolve(['日本語','🦀']));
    let rejected=false;try{api.identity([1n,2]);}catch{rejected=true;}
    let futureFailed=false;try{await api.await(Promise.resolve(['allocated',1]));}catch(e){futureFailed=!e.message.includes('deadlock');}
    const other=await instantiate({instantiateCore:async(module,imports)=>{const instance=await WebAssembly.instantiate(module,imports);if(instance.exports.live)memory=instance.exports;return instance;}});
    let closed=0,streamFailed=false;
    const source={ [Symbol.asyncIterator](){return this;},async next(){return {done:false,value:['allocated',1]};},async return(){closed++;return {done:true};} };
    try{await other.api.first(source);}catch(e){streamFailed=!e.message.includes('deadlock');}
    return {literal,snapshot,texts,matrix,future,rejected,futureFailed,streamFailed,closed,live:memory.live()};
  });
  expect(result).toEqual({literal:['40','2'],snapshot:['40','2'],texts:['','日本語🦀'],matrix:[[1,2],[],[3]],future:['日本語','🦀'],rejected:true,futureFailed:true,streamFailed:true,closed:1,live:0});
  expect(errors).toEqual([]);
});
