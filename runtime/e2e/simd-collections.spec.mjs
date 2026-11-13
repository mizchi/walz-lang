import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"]) test(`${backend}: immutable SIMD collections preserve tails, exact sums and portable cleanup`, async ({ page }) => {
  await page.goto(`/${backend}/`);
  const result = await page.evaluate(async () => {
    const { instantiate } = await import(new URL("simd-collections/index.js", location.href));
    let arena;
    const { api } = await instantiate({ instantiateCore: async (module, imports) => {
      const value = await WebAssembly.instantiate(module, imports);
      if (value.exports.live) arena = value.exports;
      return value;
    } });
    const checks = [];
    for (const n of [0, 1, 3, 4, 5, 31, 32, 33, 127, 128, 129, 257]) {
      const words = new Int32Array(Math.ceil(n / 32)).fill(-1);
      checks.push(api.count(n, words) === n);
      checks.push(api.indices(n, words).length === n);
      checks.push(api.count(n, api.complement(n, words)) === 0);
      checks.push(api.countEq(new Int32Array(n), 0) === n);
      checks.push(api.equal(new Int32Array(n), 0).length === n);
      const values = Int32Array.from({ length: n }, (_, i) => i % 2 ? -2147483648 : 2147483647);
      const expected = Array.from(values).reduce((sum, value) => sum + BigInt(value), 0n);
      checks.push(api.sum(values) === expected && api.sumWhere(values, words) === expected);
    }
    const query = api.query([10, 20, 22, 70], [1, 2, 2, 1], 10, 30, 2);
    checks.push(query.count === 2 && query.sum === 42n && query.indices.join(",") === "1,2");
    checks.push(api.countMany([10, 20, 22, 70], [10, 20, 70], [30, 22, 70]).join(",") === "3,1,0");
    let trapped = false;
    try { api.query([1], [], 0, 2, 0); } catch { trapped = true; }
    return { correct: checks.every(Boolean), trapped, live: arena.live() };
  });
  expect(result).toEqual({ correct: true, trapped: true, live: 0 });
});
