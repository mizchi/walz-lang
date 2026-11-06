import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: generic dictionary and list builder run on browser GC`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("collections/index.js", location.href).href);
      const { api } = await instantiate();
      const keys = ["a", "q", "a", "日本語🦀", "\0"];
      const values = [1n, 2n, 3n, 4n, 5n];
      const encode = xs => xs.map(x => x === undefined ? null : String(x));
      return {
        values: encode(api.mutations(keys, values, ["q"], 0)),
        cleared: encode(api.mutations(keys, values, [], 1)),
        snapshots: api.snapshots(["", "日本語🦀", "\0"]),
        entries: api.entries(keys, values).map(([k,v]) => [k,String(v)]).sort(([a],[b]) => a < b ? -1 : a > b ? 1 : 0),
      };
    });
    expect(result).toEqual({ values: ["3", null, "3", "4", "5"], cleared: [null, null, null, null, null], snapshots: [["", "日本語🦀", "\0"], ["later"]], entries: [["\0","5"],["a","3"],["q","2"],["日本語🦀","4"]] });
  });
