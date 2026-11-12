import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: checked sizes preserve full bit widths and distinguish overflow`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("checked-integers/index.js", location.href));
      const { api } = await instantiate();
      const { instantiate: layout } = await import(new URL("checked-layout/index.js", location.href));
      const { api: l } = await layout();
      return { maximum: api.add32(-1, 0), overflow: api.add32(-1, 1),
        zero: api.mul64(0n, -1n).toString(), mulOverflow: api.mul64(-1n, 2n),
        alignment: api.align64(1n, -(1n << 63n)).toString(),
        alignedMaximum: api.align32(-1, 1), badAlignment: api.align32(42, 3),
        roundingOverflow: api.align64(-1n, 2n), order: api.order(),
        main: l.main(), layout: l.layout(10, 4, 1, 2, 1024), limit: l.layout(10, 4, 1, 2, 41) };
    });
    expect(result).toEqual({ maximum: -1, overflow: undefined, zero: "0", mulOverflow: undefined,
      alignment: "-9223372036854775808", alignedMaximum: -1, badAlignment: undefined,
      roundingOverflow: undefined, order: 123456, main: 42, layout: 42, limit: undefined });
  });
