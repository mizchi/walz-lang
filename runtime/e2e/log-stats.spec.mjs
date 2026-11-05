import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"])
  test(`${backend}: the log application parses, aggregates and renders JSON in Wasm GC`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("log-stats/index.js", location.href).href
      );
      const { createTextIO, TEXT_IO } = await import("/text-io.mjs");
      const input = new TextEncoder().encode(
        "INFO\t12\t日本語🦀\r\nWARN\t7\tmsg\nERROR\t4294967295\tmax\ninvalid\n\n# comment\n",
      );
      const output = [];
      let memory;
      const { api } = await instantiate({
        imports: {
          [TEXT_IO]: createTextIO({
            input: (async function* () {
              for (let i = 0; i < input.length; i += 3)
                yield input.subarray(i, i + 3);
            })(),
            write: async (bytes) =>
              output.push(new TextDecoder().decode(bytes)),
          }),
        },
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.alloc) memory = instance.exports;
          return instance;
        },
      });
      return {
        value: await api.main(),
        report: JSON.parse(output.join("")),
        live: memory.live(),
      };
    });
    expect(result).toEqual({
      value: { tag: "ok", val: 0 },
      live: 0,
      report: {
        accepted: 3,
        rejected: 1,
        ignored: 2,
        levels: {
          INFO: { count: 1, total_ms: 12, max_ms: 12 },
          WARN: { count: 1, total_ms: 7, max_ms: 7 },
          ERROR: { count: 1, total_ms: 4294967295, max_ms: 4294967295 },
        },
      },
    });
  });
