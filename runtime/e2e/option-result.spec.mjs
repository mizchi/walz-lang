import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: core combinators and short UTF-8 strings`,async({page})=>{
 await page.goto(`/${backend}/`);
 const result=await page.evaluate(async()=>{
  const {instantiate}=await import(new URL('option-result/index.js',location.href).href);let memory;
  const {api}=await instantiate({instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memory=x.exports;return x;}});
  return {some:api.supply('日本😀'),none:api.supply(undefined),ok:api.mapped({tag:'ok',val:42n},5).map((v,i)=>i?v:{...v,val:String(v.val)}),err:api.mapped({tag:'err',val:7},5),text:[...api.literal()],same:api.identity(),live:memory.live()};
 });
 expect(result).toEqual({some:[{tag:'ok',val:'日本😀'},1],none:[{tag:'err',val:7},1],ok:[{tag:'ok',val:'42'},0],err:[{tag:'err',val:12},1],text:['日本😀','','quote"'],same:1,live:0});
});
