import { test, expect } from "@playwright/test";
for (const backend of ["core", "component"])
  test(`${backend}: browser named imports expose only the selected API without a clock provider`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("module-imports/index.js", location.href).href);
      const { api } = await instantiate();
      return { answer: api.answer(), names: Object.keys(api) };
    });
    expect(result).toEqual({ answer: 42, names: ["answer"] });
  });
