import { test, expect } from "@playwright/test";
for (const backend of ["core", "component"])
  test(`${backend}: browser wlt:core types retain Future, Result and Option ABIs`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("standard/index.js", location.href).href);
      const { api } = await instantiate({ eventLoop: { deferI64: async (_ms, value) => value } });
      return { main: await api.main(42n), missing: typeof api.optional(0), present: api.optional(1), future: await api.future() };
    });
    expect(result).toEqual({ main: { tag: "ok", val: "42" }, missing: "undefined", present: "present", future: { tag: "ok", val: "ready" } });
  });
