import { test, expect } from "@playwright/test";

for (const backend of ["component", "core"]) {
  test.describe(backend, () => {
    test("Chromium resumes GC code through real JSPI while the browser event loop progresses", async ({
      page,
    }) => {
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.goto(`/${backend}/`);
      const result = await page.evaluate(async () => {
        const { instantiate } = await import(
          new URL("event-loop/index.js", location.href).href
        );
        const { api } = await instantiate();
        const events = [];
        const first = api.main(30).then((value) => {
          events.push("resume");
          return value.toString();
        });
        const timer = new Promise((resolve) =>
          setTimeout(() => {
            events.push("timer");
            resolve();
          }, 0),
        );
        const [value] = await Promise.all([first, timer]);
        const sum = (await api.sumTicks(5, 1)).toString();
        const error = await api.completionError(17);
        return {
          value,
          sum,
          error,
          events,
          jspi: typeof WebAssembly.Suspending,
        };
      });
      expect(result).toEqual({
        value: "42",
        sum: "10",
        error: 17,
        events: ["timer", "resume"],
        jspi: "function",
      });
      expect(errors).toEqual([]);
    });

    test("browser bindings lift futures, streams and canonical variant values", async ({
      page,
    }) => {
      await page.goto(`/${backend}/`);
      const result = await page.evaluate(async () => {
        const { instantiate } = await import(
          new URL("values/index.js", location.href).href
        );
        const { api } = await instantiate();
        const stream = [];
        for await (const value of api.stream()) stream.push(value.toString());
        const future = (await api.future()).toString();
        const option = api.value({ tag: "err", val: -1.25 });
        const received = (
          await api.await(
            new Promise((resolve) => setTimeout(() => resolve(42n), 1)),
          )
        ).toString();
        const unitFuture = await api.unitFuture();
        const units = [];
        for await (const value of api.unitStream()) units.push(value);
        const options = [];
        for await (const value of api.optionStream())
          options.push(value?.toString() ?? null);
        return { stream, future, option, received, unitFuture, units, options };
      });
      expect(result).toEqual({
        stream: ["40", "2"],
        future: "42",
        option: { tag: "err", val: -1.25 },
        received: "42",
        unitFuture: undefined,
        units: [undefined],
        options: [null, "42"],
      });
    });

    test("a rejected host call settles the caller and traps the instance", async ({
      page,
    }) => {
      await page.goto(`/${backend}/`);
      const messages = await page.evaluate(async () => {
        const { instantiate } = await import(
          new URL("event-loop/index.js", location.href).href
        );
        const { api } = await instantiate();
        const messages = [];
        for (const milliseconds of [-1, 0]) {
          try {
            await api.main(milliseconds);
            messages.push("unexpected success");
          } catch (error) {
            messages.push(error.message);
          }
        }
        return messages;
      });
      expect(messages[0]).toContain("invalid timer duration");
      expect(messages[1]).toContain("cannot enter component instance");
    });

    test("browser task cancellation unwinds catch_all, closes owners and preserves the instance", async ({
      page,
    }) => {
      await page.goto(`/${backend}/`);
      const results = await page.evaluate(async () => {
        const { instantiate } = await import(
          new URL("event-loop/index.js", location.href).href
        );
        const { createEventLoop } = await import(
          new URL("event-loop/host.mjs", location.href).href
        );
        let waiting;
        let allocator;
        const { api, start } = await instantiate({
          eventLoop: createEventLoop({
            onEvent: (e) => {
              if (e.type === "timer-start" && e.milliseconds === 60000)
                waiting?.();
            },
          }),
          instantiateCore: async (module, imports) => {
            const instance = await WebAssembly.instantiate(module, imports);
            if (instance.exports.alloc) allocator = instance.exports;
            return instance;
          },
        });
        const results = [];
        for (const name of ["main", "hold", "catchAll"]) {
          const ready = new Promise((resolve) => {
            waiting = resolve;
          });
          const controller = new AbortController();
          const task = start(name, [60000], { signal: controller.signal });
          const rejection = task.result.catch((error) => error.name);
          await ready;
          controller.abort("stop");
          const done = await task.cancel();
          results.push([
            done.status,
            done.started,
            await rejection,
            allocator.live(),
            (await api.main(0)).toString(),
          ]);
        }
        return results;
      });
      expect(results).toEqual(
        Array(3).fill(["cancelled", true, "TaskCancelledError", 0, "42"]),
      );
    });

    test("browser cancellation detaches pending future and stream buffers before disposal", async ({
      page,
    }) => {
      await page.goto(`/${backend}/`);
      const results = await page.evaluate(async () => {
        const { instantiate } = await import(
          new URL("values/index.js", location.href).href
        );
        let ready;
        const { api, start } = await instantiate({
          onTaskEvent: (e) => {
            if (e.type === "read-pending") ready?.();
          },
        });
        const results = [];
        for (const name of ["await", "first"]) {
          let closed = 0;
          const dispose = async () => {
            await new Promise((resolve) => setTimeout(resolve, 0));
            closed++;
          };
          const input =
            name === "await"
              ? Object.assign(new Promise(() => {}), {
                  [Symbol.asyncDispose]: dispose,
                })
              : {
                  [Symbol.asyncIterator]() {
                    return this;
                  },
                  next() {
                    return new Promise(() => {});
                  },
                  async return() {
                    await dispose();
                    return { done: true };
                  },
                };
          const pending = new Promise((resolve) => {
            ready = resolve;
          });
          const task = start(name, [input]);
          const rejection = task.result.catch((error) => error.name);
          await pending;
          results.push([(await task.cancel()).status, await rejection, closed]);
        }
        return {
          results,
          value: (await api.await(Promise.resolve(42n))).toString(),
        };
      });
      expect(results).toEqual({
        results: Array(2).fill(["cancelled", "TaskCancelledError", 1]),
        value: "42",
      });
    });

    test("browser bytes/text ABI preserves UTF-8 and releases nested canonical allocations", async ({
      page,
    }) => {
      await page.goto(`/${backend}/`);
      const result = await page.evaluate(async () => {
        const { instantiate } = await import(
          new URL("buffers/index.js", location.href).href
        );
        let live;
        const { api } = await instantiate({
          instantiateCore: async (module, imports) => {
            const instance = await WebAssembly.instantiate(module, imports);
            if (instance.exports.live) live = instance.exports.live;
            return instance;
          },
        });
        const text = "日本語🦀\0";
        const bytes = api.encode(text);
        const stream = [];
        for await (const item of api.stream(bytes))
          stream.push(Array.from(item));
        const nested = api.nested([text, bytes]);
        return {
          text: api.text(text),
          decoded: api.decode(bytes),
          bad: api.decode(new Uint8Array([0xed, 0xa0, 0x80])),
          future: await api.ready(text),
          received: await api.await(Promise.resolve(text)),
          nested: [nested[0], Array.from(nested[1])],
          bytes: Array.from(bytes),
          stream,
          live: live(),
        };
      });
      const text = "日本語🦀\0";
      const bytes = Array.from(new TextEncoder().encode(text));
      expect(result).toEqual({
        text,
        decoded: text,
        bad: undefined,
        future: text,
        received: text,
        nested: [text, bytes],
        bytes,
        stream: [bytes],
        live: 0,
      });
    });
  });
}
