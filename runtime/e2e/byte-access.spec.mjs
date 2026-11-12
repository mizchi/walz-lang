import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: fixed-width bytes and logical view bounds in Chromium`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("byte-access/index.js", location.href));
      const { api } = await instantiate();
      const bytes = Uint8Array.of(173, 1, 128, 255, 127, 42, 0, 0, 0, 173);
      const data = new DataView(bytes.buffer);
      const actual = api.readI32Be(bytes, 1);
      const view = api.viewI32Le(bytes, 1, 4, 0);
      const written = api.writeI64Le(bytes, 1, -(1n << 63n));
      const float = api.readF64Be(api.writeF64Be(bytes, 1, -0), 1);
      // There are sufficient owner bytes, but insufficient logical view bytes.
      const { api: bad } = await instantiate();
      let trapped = false;
      try { bad.viewI32Le(bytes, 1, 3, 0); } catch (error) { trapped = error instanceof WebAssembly.RuntimeError; }
      return { actual, expected: data.getInt32(1, false), view, expectedView: data.getInt32(1, true),
        written: Array.from(written), minusZero: Object.is(float, -0), order: api.order(), trapped };
    });
    expect(result.actual).toBe(result.expected);
    expect(result.view).toBe(result.expectedView);
    expect(result.written).toEqual([173, 0, 0, 0, 0, 0, 0, 0, 128, 173]);
    expect(result.minusZero).toBe(true);
    expect(result.order).toBe(12345);
    expect(result.trapped).toBe(true);
  });
