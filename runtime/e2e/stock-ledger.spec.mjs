import { test, expect } from "@playwright/test";
for (const backend of ["core", "component"])
  test(`${backend}: stock ledger escapes dynamic keys and preserves state after business errors`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("stock-ledger/index.js", location.href).href
      );
      const { createTextIO, TEXT_IO } = await import("/text-io.mjs");
      const key = 'a"\\日本語🦀\0';
      const bytes = new TextEncoder().encode(
        `ADD	${key}	2
TAKE	${key}	5
SHOW	${key}
ADD	Aa	4
ADD	BB	5
SHOW	Aa
invalid
`,
      );
      const output = [];
      let memory;
      const { api } = await instantiate({
        imports: {
          [TEXT_IO]: createTextIO({
            input: (async function* () {
              for (let i = 0; i < bytes.length; i += 3)
                yield bytes.subarray(i, i + 3);
            })(),
            write: async (bytes) =>
              output.push(JSON.parse(new TextDecoder().decode(bytes))),
          }),
        },
        instantiateCore: async (m, i) => {
          const instance = await WebAssembly.instantiate(m, i);
          if (instance.exports.alloc) memory = instance.exports;
          return instance;
        },
      });
      return { key, result: await api.main(), output, live: memory.live() };
    });
    expect(result.result).toEqual({ tag: "ok", val: 0 });
    expect(result.live).toBe(0);
    expect(result.output).toEqual([
      { op: "ADD", key: result.key, ok: true, quantity: 2 },
      { op: "TAKE", key: result.key, ok: false, error: "insufficient-stock" },
      { op: "SHOW", key: result.key, ok: true, quantity: 2 },
      { op: "ADD", key: "Aa", ok: true, quantity: 4 },
      { op: "ADD", key: "BB", ok: true, quantity: 5 },
      { op: "SHOW", key: "Aa", ok: true, quantity: 4 },
      { ok: false, error: "invalid-command" },
    ]);
  });
