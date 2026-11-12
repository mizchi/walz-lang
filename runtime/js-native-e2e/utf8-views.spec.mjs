import { test, expect } from "@playwright/test";
for (const mode of ["utf8", "js-strings"])
  test(`${mode}: native ABI preserves strict UTF-8 range semantics`, async ({ page }) => {
    await page.goto("/");
    const result = await page.evaluate(async mode => {
      const { instantiate } = await import(new URL(`${mode}-utf8-views/index.js`, location.href));
      const { api } = await instantiate();
      const bytes = Uint8Array.of(255, ...new TextEncoder().encode("\ufeff日🦀\0"), 255);
      return { valid: api.valid(bytes), length: api.length(bytes, 1, 11),
        text: api.decode(bytes, 1, 11), view: api.text(bytes, 1, 11, 3, 3),
        invalid: api.materialize(Uint8Array.of(237, 160, 128), 0, 3),
        empty: api.decode(bytes, 0, 0), order: api.order() };
    }, mode);
    expect(result).toEqual({ valid: 0, length: 11, text: "\ufeff日🦀\0", view: "日",
      invalid: undefined, empty: "", order: 123456 });
  });
