import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: browser optimization preserves Promise results`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import(new URL('async-abi/index.js',location.href).href);
    const {api}=await instantiate();const value=api.main();
    return {promise:value instanceof Promise,value:String(await value)};
  });
  expect(result).toEqual({promise:true,value:'42'});
});
