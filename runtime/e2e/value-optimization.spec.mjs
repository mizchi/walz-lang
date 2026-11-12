import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"]) {
  test(`${backend}: scalarized values retain tags, payload identity, traps and ABI results`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const value = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("value-optimization/index.js", location.href));
      let arena;
      const options = { instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } };
      const { api } = await instantiate(options);
      const edges = [0, 1, 2, 65535, 65536, 0x7fffffff, 0x80000000, 0xffffffff];
      for (const a of edges) for (const b of edges) for (const h of edges) {
        const total = BigInt(a) * BigInt(b) + BigInt(h);
        const expected = total > 0xffffffffn ? -1 : Number(BigInt.asIntN(32, total));
        if (api.layout(a | 0, b | 0, h | 0) !== expected) throw new Error("layout mismatch");
      }
      const result = { identity: api.identity(), unit: [api.unit(0), api.unit(1)],
        null: [api.null(0), api.null(1)], caught: [api.caught(0), api.caught(1)],
        handler: api.handler(), escaped: api.escaped(42), main: api.main(), live: arena.live() };
      const bad = await instantiate(options);
      try { bad.api.ignored(0); result.trapped = false; }
      catch (e) { result.trapped = e instanceof WebAssembly.RuntimeError; }
      result.afterTrap = arena.live();
      return result;
    });
    expect(value).toEqual({ identity: 43, unit: [10, 20], null: [10, 21], caught: [42, -7],
      handler: 7, escaped: { tag: "ok", val: 42 }, main: 42, live: 0, trapped: true, afterTrap: 0 });
  });

  test(`${backend}: discarded async payloads still execute before completion and cancellation`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const value = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("value-optimization-async/index.js", location.href));
      let calls = 0, disposed = 0, arena;
      let entered;
      const started = new Promise(resolve => { entered = resolve; });
      const options = { eventLoop: { sleep(ms) {
        calls++;
        if (ms === 0) return Promise.resolve();
        if (ms === 1) return Promise.reject(new Error("payload rejected"));
        let resolve;
        const promise = new Promise(r => { resolve = r; });
        promise[Symbol.asyncDispose] = async () => { disposed++; resolve(); };
        entered(); return promise;
      } }, instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } };
      const first = await instantiate(options);
      const answer = await first.api.main(0);
      let rejected = false;
      try { await first.api.main(1); } catch (e) { rejected = e.message.includes("payload rejected"); }
      const failedLive = arena.live();
      const second = await instantiate(options), task = second.start("main", [2]);
      const cancelled = task.result.then(() => false, e => e.name === "TaskCancelledError");
      await started; await task.cancel();
      return { answer: String(answer), rejected, failedLive, cancelled: await cancelled, calls, disposed, live: arena.live() };
    });
    expect(value).toEqual({ answer: "42", rejected: true, failedLive: 0, cancelled: true, calls: 3, disposed: 1, live: 0 });
  });
}
