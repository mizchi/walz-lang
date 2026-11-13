import {test,expect} from '@playwright/test';
test('explicit binary instances share aliases and isolate state, copy values, and ship without source files',async({page})=>{
  await page.goto('/component/');
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import('/component/wasm-use/index.js');
    const arenas=[];
    const {api}=await instantiate({instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);
      if(instance.exports.live)arenas.push(instance.exports);
      return instance;
    }});
    const states=[await api.main(),await api.main()];
    const independent=await api.otherNext();
    const double=String(await api.double(21n));
    const consumed=String(await api.consume(Promise.resolve(42n)));
    const later=String(await(await api.later(42n)));
    const text=await api.text('WIT 日本語😀');
    const data=[...await api.data(Uint8Array.of(0,128,255))];
    const point=await api.point({x:1,y:2});
    const option=await api.optional(42);
    let error;try{await api.result({tag:'err',val:'invalid'});}catch(failure){error=failure.payload;}
    const again=await api.main(),future=String(await api.localFuture());
    await new Promise(resolve=>setTimeout(resolve,25));
    return{states,independent,double,consumed,later,again,text,data,point,option,error,future,live:arenas.map(a=>a.live())};
  });
  expect(result.states).toEqual([3,7]);expect(result.independent).toBe(1);expect(result.double).toBe('42');expect(result.consumed).toBe('42');expect(result.later).toBe('42');expect(result.again).toBe(11);
  expect(result.text).toBe('WIT 日本語😀');expect(result.data).toEqual([0,128,255]);
  expect(result.point).toEqual({x:1,y:2});expect(result.option).toBe(42);
  expect(result.error).toBe('invalid');
  expect(result.future).toBe('42');
  expect(result.live.length).toBeGreaterThanOrEqual(2);expect(result.live).toEqual(result.live.map(()=>0));
});
