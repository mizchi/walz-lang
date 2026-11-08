import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: frozen succinct indexes share data across Workers and join cancellation`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const root=new URL('succinct/',location.href);
    const {instantiate}=await import(new URL('sdk/index.mjs',root));
    const sdk=await instantiate(root);
    const input=Uint8Array.from({length:4097},(_,i)=>i%256);
    const index=sdk.buildWavelet(input);
    input.fill(0);
    try{
      const q=Int32Array.from([3,0,4097,(0|(128<<16)),4,0,4097,2048]);
      await index.prepare({workers:2});
      const answer=[...await index.queryMany(q,{workers:2})];
      const abort=new AbortController(),many=new Int32Array(4*100000);
      for(let i=0;i<100000;i++)many.set([4,0,4097,i%4097],i*4);
      const pending=index.queryMany(many,{workers:2,signal:abort.signal});
      queueMicrotask(()=>abort.abort());
      let cancelled=false;
      try{await pending;}catch(error){cancelled=error.name==='AbortError';}
      return{answer,cancelled,again:[...await index.queryMany(q,{workers:2})]};
    }finally{await index.close();await index.close();}
  });
  expect(result).toEqual({answer:[2049,127],cancelled:true,again:[2049,127]});
});
