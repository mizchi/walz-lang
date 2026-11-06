import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: bulk SIMD kernels preserve tails and portable ABI cleanup`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("simd-kernels/index.js", location.href).href);
      let arena;
      const { api } = await instantiate({ instantiateCore: async (m, i) => {
        const instance = await WebAssembly.instantiate(m, i);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } });
      const cases = [];
      for (const n of [0, 1, 3, 4, 5, 15, 16, 17]) {
        const a = Int32Array.from({ length: n }, (_, i) => i % 2 ? -2147483648 : 2147483647);
        const bytes = Uint8Array.from({ length: n }, (_, i) => i % 251 + 1);
        const sum = Array.from(a).reduce((n, x) => n + BigInt(x), 0n);
        cases.push(api.sumSimd(a) === sum && api.sumScalar(a) === sum);
        cases.push(api.indexSimd(bytes, 0) === -1 && api.indexScalar(bytes, 0) === -1);
        cases.push(api.indexSimd(bytes, 1) === bytes.indexOf(1));
      }
      const dot = api.dotSimd([1, 2, 3, 4, 5], [2, 4, 6, 8, 10]);
      const bitmap = api.bitmapSimd([-1, 15, 0, 8, 1], [3, 7, 9, 8, 1]);
      let trapped = false;
      try { api.dotSimd([1], []); } catch { trapped = true; }
      return {
        cases: cases.every(Boolean), trapped,
        dot, bitmap,
        live: arena.live(),
      };
    });
    expect(result).toEqual({ cases: true, trapped: true, dot: 110, bitmap: 7, live: 0 });
  });
