import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: SIMD survives GC and a JSPI suspension in Chromium`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("simd/index.js", location.href).href);
      let yields = 0, arena;
      const { api } = await instantiate({
        eventLoop: { yield: async () => {
          yields++;
          await new Promise(resolve => setTimeout(resolve, 1));
        } },
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        },
      });
      const a = [2147483647, -2147483648, 3, -4], b = [2, -1, 5, 6];
      return {
        add: api.add(2147483647, 1),
        dot: api.dot(a, b),
        shuffle: Array.from(api.shuffle(a, b)),
        suspended: await api.suspended(-2147483648),
        ready: await api.ready(), yields,
        live: arena.live(),
      };
    });
    expect(result).toEqual({
      add: -2147483648, dot: 2147483637,
      shuffle: [2, 2147483647, -1, -2147483648],
      suspended: -2147483648, ready: 42, yields: 1, live: 0,
    });
  });
