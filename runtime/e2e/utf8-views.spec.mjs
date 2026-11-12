import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: UTF-8 bytes views validate only their range in Chromium`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("utf8-views/index.js", location.href));
      const { api } = await instantiate();
      const content = new TextEncoder().encode("\ufeff日🦀\0");
      const bytes = Uint8Array.of(255, ...content, 255);
      const { instantiate: views } = await import(new URL("views/index.js", location.href));
      const scope = await views();
      const after = await scope.api.afterBytes(bytes, 1, content.length, new Promise(r => setTimeout(r, 0)));
      const task = scope.start("afterBytes", [bytes, 1, content.length, new Promise(() => {})]);
      const failed = task.result.then(() => "unexpected", e => e.name);
      await new Promise(r => setTimeout(r, 10));
      const done = await task.cancel();
      const { api: bad } = await instantiate();
      let trapped = false;
      try { bad.validRange(bytes, -1, 0); } catch (e) { trapped = e instanceof WebAssembly.RuntimeError; }
      return { valid: api.valid(bytes), range: api.validRange(bytes, 1, content.length),
        length: api.length(bytes, 1, content.length), text: api.text(bytes, 1, content.length, 3, 3),
        owned: api.decode(bytes, 1, content.length), truncated: api.length(Uint8Array.of(240, 159, 166, 128), 0, 3),
        empty: api.decode(Uint8Array.of(255), 0, 0), order: api.order(),
        after, cancelled: await failed, status: done.status, trapped };
    });
    expect(result).toEqual({ valid: 0, range: 1, length: 11, text: "日", owned: "\ufeff日🦀\0",
      truncated: undefined, empty: "", order: 123456, after: "\ufeff日🦀\0",
      cancelled: "TaskCancelledError", status: "cancelled", trapped: true });
  });
