import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"]) test(`${backend}: dictionary columns preserve UTF-8 keys, code order, SIMD selection and cleanup`, async ({ page }) => {
  await page.goto(`/${backend}/`);
  const result = await page.evaluate(async () => {
    const { instantiate } = await import(new URL("dictionary-column/index.js", location.href));
    let arena;
    const { api } = await instantiate({ instantiateCore: async (module, imports) => {
      const value = await WebAssembly.instantiate(module, imports);
      if (value.exports.live) arena = value.exports;
      return value;
    } });
    const values = ["JP", "日本🦀", "JP", "", "\u0000", "é", "e\u0301"];
    const snapshot = api.encode(values);
    const counts = api.countMany(values, ["JP", "missing", "JP", "", "\u0000"]);
    const rows = api.select(values, ["JP", "JP", "missing"]);
    const query = api.query(["JP", "US", "JP"], [20, 100, 22], ["JP"], 10, 30);
    const fused = api.query(["JP", "US", "JP", "JP", "JP"],
      [2147483646, 0, 2147483646, -2147483648, 2147483647], ["JP", "JP", "missing"], 2147483646, 2147483647);
    const composed = api.queryComposed(["JP", "US", "JP", "JP", "JP"],
      [2147483646, 0, 2147483646, -2147483648, 2147483647], ["JP", "JP", "missing"], 2147483646, 2147483647);
    const tail = api.query(["JP", "JP", "JP"], [0, 0, 0], ["JP"], -1, 1);
    const decoded = api.decode(snapshot);
    const empty = api.encode([]);
    let trapped = false;
    try { api.decode({ dictionary: ["duplicate", "duplicate"], codes: [] }); } catch { trapped = true; }
    return { dictionary: snapshot.dictionary, codes: Array.from(snapshot.codes),
      decoded, counts: Array.from(counts), rows: Array.from(rows),
      query: [query.count, String(query.sum), Array.from(query.indices)],
      fused: [fused.count, String(fused.sum), Array.from(fused.indices)],
      composed: [composed.count, String(composed.sum), Array.from(composed.indices)],
      tail: [tail.count, String(tail.sum), Array.from(tail.indices)],
      empty: [empty.dictionary, Array.from(empty.codes)], trapped, live: arena.live() };
  });
  expect(result).toEqual({ dictionary: ["JP", "日本🦀", "", "\u0000", "é", "e\u0301"],
    codes: [0, 1, 0, 2, 3, 4, 5], decoded: ["JP", "日本🦀", "JP", "", "\u0000", "é", "e\u0301"],
    counts: [2, 0, 2, 1, 1], rows: [0, 2], query: [2, "42", [0, 2]],
    fused: [2, "4294967292", [0, 2]], composed: [2, "4294967292", [0, 2]], tail: [3, "0", [0, 1, 2]],
    empty: [[], []], trapped: true, live: 0 });
});
