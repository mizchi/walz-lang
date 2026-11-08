import {test,expect} from '@playwright/test';
test('WIT binary uses share state, copy values, and ship without source files',async({page})=>{
  await page.goto('/component/');
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import('/component/wasm-use/index.js');
    const arenas=[];
    const {api,start}=await instantiate({instantiateCore:async(module,imports)=>{
      const instance=await WebAssembly.instantiate(module,imports);
      if(instance.exports.live)arenas.push(instance.exports);
      return instance;
    }});
    const first=start('main'),second=start('main');
    await Promise.resolve();const queued=second.state==='queued';
    const states=[await first.result,await second.result];
    const task=start('main');let cancelled=false;
    const rejected=task.result.catch(error=>cancelled=error.name==='TaskCancelledError');
    await task.cancel();await rejected;
    const text=await api.text('WIT 日本語😀');
    const data=[...await api.data(Uint8Array.of(0,128,255))];
    const point=await api.point({x:1,y:2});
    const option=await api.optional(42);
    const error=await api.result({tag:'err',val:'invalid'});
    return{states,queued,cancelled,again:await api.main(),text,data,point,option,error,future:String(await api.localFuture()),live:arenas.map(a=>a.live())};
  });
  expect(result.states).toEqual([3,7]);expect(result.queued).toBe(true);expect(result.cancelled).toBe(true);expect(result.again).toBe(11);
  expect(result.text).toBe('WIT 日本語😀');expect(result.data).toEqual([0,128,255]);
  expect(result.point).toEqual({x:1,y:2});expect(result.option).toBe(42);
  expect(result.error).toEqual({tag:'err',val:'invalid'});
  expect(result.future).toBe('42');
  expect(result.live.length).toBeGreaterThanOrEqual(2);expect(result.live.every(n=>n===0)).toBe(true);
});
