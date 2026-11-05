import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: browser resource cleanup`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate,TaskCancelledError}=await import(new URL('resources/index.js',location.href).href);
    let opened=0,closed=0,pending,stopped=false;
    const {api,start}=await instantiate({imports:{files:{open:()=>({id:++opened}),read:f=>f.id,close:f=>{if(f.id===4&&!stopped)throw new Error("closed before abort disposal");closed++;}}},eventLoop:{sleep:()=>{pending=Promise.withResolvers();pending.promise[Symbol.asyncDispose]=async()=>{pending.reject(new Error("aborted"));await Promise.resolve();stopped=true;};return pending.promise;}}});
    const normal=api.normal(),err=api.err(),caught=api.caught();
    const task=start('pending');for(let i=0;i<50&&!pending;i++)await Promise.resolve();
    await task.cancel();let cancelled=false;try{await task.result;}catch(e){cancelled=e instanceof TaskCancelledError;}
    const trap=start('trap');try{await trap.result;}catch{}
    return {normal,err,caught,cancelled,opened,closed};
  });
  expect(result).toEqual({normal:1,err:{tag:'err',val:7},caught:42,cancelled:true,opened:5,closed:5});
});
