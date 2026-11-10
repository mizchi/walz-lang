import {test,expect} from '@playwright/test';
test('general WorkerHost runs asynchronous jobs and transferable data',async({page})=>{
  await page.goto('/');
  const result=await page.evaluate(async()=>{
    const {WorkerHost}=await import('/workers/host.mjs');
    const host=await WorkerHost.create({workers:2,driver:new URL('/workers/worker-driver.mjs',location.href),init:{bias:2}});
    try{
      const [a,b]=await Promise.all([host.run({numbers:[20,20],delay:1}),host.run({numbers:[40]})]);
      const arrays=await host.map([{bytes:Uint8Array.of(1,2)}]);
      return{a,b,bytes:[...arrays[0]]};
    }finally{await host.close();}
  });
  expect(result).toEqual({a:42,b:42,bytes:[3,4]});
});
for(const backend of ['core','component'])test(`${backend}: general Workers run a different async Walz program`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {WorkerHost}=await import('/workers/host.mjs');
    const host=await WorkerHost.create({workers:2,driver:new URL('/workers/worker-walz-driver.mjs',location.href),init:{url:new URL('worker-job/index.js',location.href).href}});
    try{
      const abort=new AbortController();
      const cancelled=host.run({value:1n,ms:10000},{signal:abort.signal});
      const sibling=host.run({value:21n,ms:1});abort.abort();
      let stopped=false;try{await cancelled;}catch(error){stopped=error.name==='AbortError';}
      return{stopped,value:String(await sibling)};
    }finally{await host.close();}
  });
  expect(result).toEqual({stopped:true,value:'42'});
});
