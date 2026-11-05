import { test, expect } from "@playwright/test";

for (const backend of ["core", "component"]) {
  test(`${backend}: browser UTF-8 capabilities filter lines and report decoding errors`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("text-io/index.js", location.href).href
      );
      const { createTextIO, TEXT_IO } = await import("/text-io.mjs");
      const bytes = new TextEncoder().encode("日本語🦀\r\n\nlast");
      const output = [];
      let allocator;
      const { api } = await instantiate({
        imports: {
          [TEXT_IO]: createTextIO({
            input: (async function* () {
              for (const b of bytes) yield new Uint8Array([b]);
            })(),
            write: async (data) => output.push(new TextDecoder().decode(data)),
          }),
        },
        instantiateCore: async (m, i) => {
          const instance = await WebAssembly.instantiate(m, i);
          if (instance.exports.alloc) allocator = instance.exports;
          return instance;
        },
      });
      const value = await api.main();
      const { api: bad } = await instantiate({
        imports: {
          [TEXT_IO]: createTextIO({
            input: (async function* () {
              yield new Uint8Array([0xff]);
            })(),
            write: async () => {
              throw new Error("unexpected output");
            },
          }),
        },
      });
      return { value, output, live: allocator.live(), bad: await bad.main() };
    });
    expect(result).toEqual({
      value: { tag: "ok", val: 2 },
      output: ["日本語🦀\n", "last\n"],
      live: 0,
      bad: { tag: "err", val: 1 },
    });
  });
  test(`${backend}: browser text I/O cancellation awaits provider cleanup`, async ({
    page,
  }) => {
    await page.goto(`/${backend}/`);
    const result = await page.evaluate(async () => {
      const { instantiate } = await import(
        new URL("text-io/index.js", location.href).href
      );
      const { createTextIO, TEXT_IO } = await import("/text-io.mjs");
      const results = [];
      for (const phase of ["read", "write"]) {
        const entered = Promise.withResolvers(),
          disposing = Promise.withResolvers(),
          release = Promise.withResolvers();
        let closed = 0,
          allocator;
        const input =
          phase === "read"
            ? {
                [Symbol.asyncIterator]() {
                  return this;
                },
                next() {
                  entered.resolve();
                  return new Promise(() => {});
                },
                async return() {
                  closed++;
                  disposing.resolve();
                  await release.promise;
                  return { done: true };
                },
              }
            : (async function* () {
                try {
                  yield new TextEncoder().encode("pending\n");
                } finally {
                  closed++;
                }
              })();
        const write =
          phase === "read"
            ? async () => {}
            : async (data, signal) => {
                entered.resolve();
                await new Promise((resolve) =>
                  signal.addEventListener(
                    "abort",
                    () => {
                      disposing.resolve();
                      resolve();
                    },
                    { once: true },
                  ),
                );
                await release.promise;
              };
        const { start } = await instantiate({
          imports: { [TEXT_IO]: createTextIO({ input, write }) },
          instantiateCore: async (m, i) => {
            const instance = await WebAssembly.instantiate(m, i);
            if (instance.exports.alloc) allocator = instance.exports;
            return instance;
          },
        });
        const task = start("main");
        const rejected = task.result.catch((error) => error.name);
        await entered.promise;
        let acknowledged = false;
        const cancelled = task.cancel().then((result) => {
          acknowledged = true;
          return result;
        });
        await disposing.promise;
        const early = acknowledged;
        release.resolve();
        results.push({
          early,
          status: (await cancelled).status,
          error: await rejected,
          closed,
          live: allocator.live(),
        });
      }
      return results;
    });
    expect(result).toEqual(
      Array(2).fill({
        early: false,
        status: "cancelled",
        error: "TaskCancelledError",
        closed: 1,
        live: 0,
      }),
    );
  });
}
