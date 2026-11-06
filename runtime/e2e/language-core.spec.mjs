import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: scalar newtypes and flat Result sequencing`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import(new URL('language-core/index.js',location.href).href);
    const {api}=await instantiate();
    return {ids:[...api.ids([42n,-1n])].map(String),plain:[...api.ints([1n])].map(String),
      future:String(await api.readyId(42n)),plainFuture:String(await api.readyInt(7n)),
      ok:String(api.compute(0).val),err:api.compute(1),sqrt:api.sqrt(1764),negative:api.sqrt(-1)};
  });
  expect(result).toEqual({ids:['42','-1'],plain:['1'],future:'42',plainFuture:'7',ok:'42',err:{tag:'err',val:7},sqrt:42,negative:0});
});
