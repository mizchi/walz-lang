import {test,expect} from '@playwright/test';
for(const backend of ['core','component']) test(`${backend}: browser WIT values and cancellation cleanup`, async({page})=>{
  await page.goto(`/${backend}/`);
  const result = await page.evaluate(async()=>{
    const {instantiate,TaskCancelledError}=await import(new URL('wit-bindings/index.js',location.href).href);
    let disposed=0, pending;
    const {api,start}=await instantiate({imports:{'example:host/service@1.0.0':{
      echo:value=>value,
      wait:()=>{pending=Promise.withResolvers(); pending.promise[Symbol.asyncDispose]=()=>{disposed++;pending.resolve('stopped');};return pending.promise;},
    }}});
    const value=api.echo({tag:'ok',val:[42n,'日本語']});
    const task=start('wait');
    for(let i=0;i<20&&!pending;i++)await Promise.resolve();
    await task.cancel('stop');
    let cancelled=false;try{await task.result;}catch(e){cancelled=e instanceof TaskCancelledError;}
    return {number:String(value.val[0]),text:value.val[1],disposed,cancelled};
  });
  expect(result).toEqual({number:'42',text:'日本語',disposed:1,cancelled:true});
});
