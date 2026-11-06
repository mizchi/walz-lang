import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: views preserve UTF-8 and GC roots across a suspension`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("views/index.js", location.href).href);
      const { api } = await instantiate();
      const text = "a".repeat(65536) + "🦀tail";
      return {
        bytes: Array.from(api.bytes(new Uint8Array([0, 255, 128, 9, 7]), 1, 4, 1, 2)),
        text: api.text("x日🦀z", 1, 7, 3, 4),
        empty: api.text("é", 2, 0, 0, 0),
        length: api.length("x日🦀z", 1, 7),
        after: await api.after(text, 65536, 4, new Promise(resolve => setTimeout(resolve, 0))),
      };
    });
    expect(result).toEqual({ bytes: [128, 9], text: "🦀", empty: "", length: 0, after: "🦀" });
  });
