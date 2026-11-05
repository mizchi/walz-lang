import { test, expect } from "@playwright/test";
for (const backend of ["core", "component"])
  test(`${backend}: browser workflow preserves newly-ready priorities, cycles and ABI cleanup`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("workflow/index.js", location.href).href
      );
      const { createTextIO, TEXT_IO } = await import("/text-io.mjs");
      const lines = [
        "TASK\tc\t1",
        "TASK\ta\t1",
        "TASK\tb\t2",
        "TASK\td\t1",
        "AFTER\tc\ta",
        "PLAN\t2",
        "AFTER\ta\tc",
        "PLAN\t2",
      ];
      const bytes = new TextEncoder().encode(lines.join("\n") + "\n"),
        output = [];
      let arena,
        closed = 0;
      const { api } = await instantiate({
        imports: {
          [TEXT_IO]: createTextIO({
            input: (async function* () {
              try {
                for (let i = 0; i < bytes.length; i += 3)
                  yield bytes.subarray(i, i + 3);
              } finally {
                closed++;
              }
            })(),
            write: async (bytes) =>
              output.push(JSON.parse(new TextDecoder().decode(bytes))),
          }),
        },
        instantiateCore: async (m, i) => {
          const instance = await WebAssembly.instantiate(m, i);
          if (instance.exports.alloc) arena = instance.exports;
          return instance;
        },
      });
      return { value: await api.main(), output, closed, live: arena.live() };
    });
    expect(result).toEqual({
      value: { tag: "ok", val: 8 },
      closed: 1,
      live: 0,
      output: [
        { line: 6, ok: true, kind: "plan", workers: 2, tasks: 4, makespan: 3 },
        { line: 6, kind: "job", id: "c", worker: 0, start: 1, finish: 2 },
        { line: 6, kind: "job", id: "a", worker: 0, start: 0, finish: 1 },
        { line: 6, kind: "job", id: "b", worker: 1, start: 0, finish: 2 },
        { line: 6, kind: "job", id: "d", worker: 0, start: 2, finish: 3 },
        { line: 8, ok: false, error: "cycle", detail: "c" },
      ],
    });
  });
