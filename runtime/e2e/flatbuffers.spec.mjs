import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"]) for (const bundled of [false, true])
  test(`${backend}: ${bundled ? "bundled" : "direct"} FlatBuffers loads actual browser Wasm assets`, async ({ page }) => {
    const errors = []; page.on("pageerror", error => errors.push(error.message));
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async bundled => {
      let path = "flatbuffers/index.js";
      if (bundled) {
        const { entry } = await (await fetch(new URL("flatbuffers-bundled/entry.json", location.href))).json();
        path = "flatbuffers-bundled/" + entry;
      }
      const { instantiate } = await import(new URL(path, location.href));
      let arena;
      const { api } = await instantiate({ instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } });
      const input = { id: 9007199254740993n, name: "日本語🦀\0", values: Int32Array.from({ length: 1537 }, (_, i) => i - 768),
        payload: Uint8Array.from({ length: 129 }, (_, i) => i), enabled: false, weight: -1.25, child: { count: 42, label: "child" } };
      const encoded = api.encode(input), decoded = api.decode(encoded);
      return { main: api.main(), tag: decoded.tag, id: decoded.val.id.toString(), name: decoded.val.name,
        values: Array.from(decoded.val.values), payload: Array.from(decoded.val.payload), enabled: decoded.val.enabled,
        weight: decoded.val.weight, child: decoded.val.child, invalid: api.decode(new Uint8Array()), live: arena.live() };
    }, bundled);
    expect(result).toEqual({ main: 42, tag: "ok", id: "9007199254740993", name: "日本語🦀\0",
      values: Array.from({ length: 1537 }, (_, i) => i - 768), payload: Array.from({ length: 129 }, (_, i) => i),
      enabled: false, weight: -1.25, child: { count: 42, label: "child" }, invalid: { tag: "err", val: 1 }, live: 0 });
    expect(errors).toEqual([]);
  });

for (const backend of ["core", "component"]) test(`${backend}: packed copies and FlatBuffers pipelines clean up in browsers`, async ({page}) => {
  await page.goto(`/${backend}/`);
  const result = await page.evaluate(async () => {
    const {instantiate:packed} = await import(new URL("packed-arrays/index.js",location.href));
    const {api:p} = await packed();
    const snapshot = p.snapshot(Uint8Array.of(1,128,255),1,2);
    const {instantiate} = await import(new URL("flatbuffers-pipeline/index.js",location.href));
    let arena;
    const {api} = await instantiate({instantiateCore:async(m,i)=>{
      const instance=await WebAssembly.instantiate(m,i);if(instance.exports.live)arena=instance.exports;return instance;
    }});
    const bytes=api.encode({id:42n,name:"🦀",values:Int32Array.of(2147483647,-1),payload:new Uint8Array(),enabled:true,weight:0,child:undefined});
    const summaries=api.inspectBatch([bytes,new Uint8Array()]);
    const changed=api.transformBatch([bytes],1), value=api.decode(changed[0].val);
    return {snapshot:Array.from(snapshot),overlap:Array.from(p.overlap()),order:p.order(),
      sum:summaries[0].val.sum.toString(),count:summaries[0].val.count,error:summaries[1],
      values:Array.from(value.val.values),live:arena.live()};
  });
  expect(result).toEqual({snapshot:[128,255],overlap:[0,0,1,2,3,4],order:12345,
    sum:"2147483646",count:2,error:{tag:"err",val:1},values:[-2147483648,0],live:0});
});
