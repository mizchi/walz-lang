import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: browser JSON parser and value construction`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import(new URL('json-value/index.js',location.href).href);
    const {api}=await instantiate();
    const source='{"values":[null,true,1.25e2,"日本語\\uD83E\\uDD80"]}';
    return {value:JSON.parse(api.normalize(source).val),constructed:JSON.parse(api.constructed().val),builder:JSON.parse(api.builder().val),error:api.normalize('[1,]')};
  });
  expect(result).toEqual({value:{values:[null,true,125,'日本語🦀']},constructed:{values:['日本語',null],n:125},builder:{nothing:null,n:125,values:[null,true]},error:{tag:'err',val:3}});
});
