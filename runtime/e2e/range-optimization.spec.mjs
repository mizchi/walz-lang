import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: range proofs preserve nested views, loop calls and traps`, async ({ page }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(new URL("range-optimization/index.js", location.href));
      const make = async () => {
        let arena;
        const value = await instantiate({ instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        } });
        return { ...value, clean: () => arena.live() === 0 };
      };
      const { api, clean } = await make();
      const bytes = Uint8Array.from({ length: 257 }, (_, i) => (i * 31 + 7) & 255);
      const view = new DataView(bytes.buffer);
      let checked = 0;
      for (let p = 0; p <= bytes.length; p++) {
        const length = bytes.length - p;
        let sum = 0n;
        for (let i = 0; i + 4 <= length; i += 4) sum += BigInt(view.getInt32(p + i, true));
        if (api.scan(bytes, p, length) !== sum || api.repeat(bytes, p, length) !== 6n * sum)
          throw new Error(`range sum ${p}`);
        if (length > 0 && api.nested(bytes, p, length, 1, length - 1) !== api.scan(bytes, p + 1, length - 1))
          throw new Error(`nested slice ${p}`);
        checked++;
      }
      const normalClean = clean();
      // Each trap gets a fresh Component instance (a trap can poison it).
      const trapped = [];
      for (const args of [[bytes, -1, 4], [bytes, 255, 4], [bytes, 0, -1]]) {
        const value = await make();
        try { value.api.scan(...args); trapped.push(false); }
        catch (e) { trapped.push(e instanceof WebAssembly.RuntimeError); }
        if (!value.clean()) throw new Error("trap leaked ABI buffer");
      }
      const nested = await make();
      try { nested.api.nested(bytes, 4, 4, 1, 4); trapped.push(false); }
      catch (e) { trapped.push(e instanceof WebAssembly.RuntimeError); }
      if (!nested.clean()) throw new Error("nested trap leaked ABI buffer");
      return { checked, normalClean, trapped };
    });
    expect(result).toEqual({ checked: 258, normalClean: true, trapped: [true, true, true, true] });
  });
