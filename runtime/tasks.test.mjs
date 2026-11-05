import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
const { build } = await import(
  process.env.GCWAT_BACKEND === "core" ? "./build-core.mjs" : "./build.mjs"
);
import { createEventLoop } from "./host.mjs";

function barrier() {
  let resolve;
  const promise = new Promise((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
async function fixture(name, fn) {
  const dir = await mkdtemp(join(tmpdir(), "gcwat-cancel-"));
  try {
    await build(`tests/component-fixtures/${name}.gwat`, dir);
    await fn(await import(pathToFileURL(join(dir, "index.js"))));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

test(
  "queued cancellation never enters the guest; completion wins over a later cancel",
  { timeout: 30000 },
  async () => {
    await fixture("event-loop", async (runtime) => {
      const events = [];
      const { api, start } = await runtime.instantiate({
        eventLoop: createEventLoop({ onEvent: (e) => events.push(e) }),
      });
      const signal = AbortSignal.abort("not needed");
      assert.throws(() => start("constructor", []), /unknown export/);
      assert.throws(() => start("main", {}), /arguments must be an array/);
      const cancelled = start("main", [60000], { signal });
      await assert.rejects(
        cancelled.result,
        (e) => e.name === "TaskCancelledError" && e.cause === "not needed",
      );
      assert.deepEqual(await cancelled.cancel(), {
        status: "cancelled",
        started: false,
      });
      assert.equal(events.length, 0);
      const returned = start("main", [0]);
      assert.equal(await returned.result, 42n);
      assert.deepEqual(await returned.cancel(), {
        status: "returned",
        started: true,
      });
      assert.equal(await api.main(0), 42n);
      const future = start("pending", [0]);
      assert.deepEqual(await future.cancel(), {
        status: "cancelled",
        started: false,
      });
      const transferred = start("pending", [0]);
      await new Promise((resolve) => queueMicrotask(resolve));
      assert.deepEqual(await transferred.cancel(), {
        status: "returned",
        started: true,
      });
      const endpoint = await transferred.result;
      assert.equal(typeof endpoint.then, "function");
      assert.equal(await endpoint, 42n);
    });
  },
);

test(
  "running cancellation acknowledges cleanup before reusing the instance, including unread owners and catch_all",
  { timeout: 30000 },
  async () => {
    await fixture("event-loop", async (runtime) => {
      for (const name of ["main", "hold", "catchAll"]) {
        const waiting = barrier();
        const events = [];
        let allocator;
        const { api, start } = await runtime.instantiate({
          eventLoop: createEventLoop({
            onEvent: (e) => {
              events.push(e);
              if (e.type === "timer-start" && e.milliseconds === 60000)
                waiting.resolve();
            },
          }),
          instantiateCore: async (module, imports) => {
            const instance = await WebAssembly.instantiate(module, imports);
            if (instance.exports.alloc) allocator = instance.exports;
            return instance;
          },
        });
        const task = start(name, [60000]);
        const rejected = assert.rejects(
          task.result,
          (e) => e.name === "TaskCancelledError",
        );
        await waiting.promise;
        assert.throws(() => api.pending(0), /component is busy/);
        const queued = start("main", [12345]);
        const queuedRejected = assert.rejects(
          queued.result,
          (e) => e.name === "TaskCancelledError",
        );
        assert.deepEqual(await queued.cancel(), {
          status: "cancelled",
          started: false,
        });
        await queuedRejected;
        assert.ok(
          !events.some(
            (e) => e.type === "timer-start" && e.milliseconds === 12345,
          ),
        );
        const done = task.cancel("stop");
        assert.equal(task.state, "cancelling");
        assert.equal(task.cancel("again"), done);
        assert.deepEqual(await done, { status: "cancelled", started: true });
        await rejected;
        assert.equal(task.state, "cancelled");
        assert.ok(events.some((e) => e.type === "dispose"));
        assert.equal(allocator.live(), 0);
        assert.equal(await api.main(0), 42n);
      }
    });
  },
);

test(
  "repeated cancellations reclaim buffers, ignore late completion and report failed disposal",
  { timeout: 30000 },
  async () => {
    await fixture("values", async (runtime) => {
      let ready, allocator;
      const { api, start } = await runtime.instantiate({
        onTaskEvent: (e) => {
          if (e.type === "read-pending") ready?.();
        },
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.alloc) allocator = instance.exports;
          return instance;
        },
      });
      const bytes = allocator.memory.buffer.byteLength;
      for (let i = 0; i < 100; i++) {
        let complete;
        const input = Object.assign(
          new Promise((resolve) => {
            complete = resolve;
          }),
          { [Symbol.asyncDispose]: () => {} },
        );
        const reading = new Promise((resolve) => {
          ready = resolve;
        });
        const task = start("await", [input]);
        const rejected = assert.rejects(
          task.result,
          (e) => e.name === "TaskCancelledError",
        );
        await reading;
        assert.equal((await task.cancel()).status, "cancelled");
        await rejected;
        complete(17n);
        assert.equal(await api.await(Promise.resolve(42n)), 42n);
        assert.equal(allocator.live(), 0);
      }
      assert.equal(allocator.memory.buffer.byteLength, bytes);
      const reading = new Promise((resolve) => {
        ready = resolve;
      });
      const bad = Object.assign(new Promise(() => {}), {
        [Symbol.asyncDispose]: async () => {
          throw new Error("close failed");
        },
      });
      const task = start("await", [bad]);
      const rejected = assert.rejects(task.result, /task cleanup failed/);
      await reading;
      assert.deepEqual(await task.cancel(), {
        status: "failed",
        started: true,
      });
      await rejected;
      assert.throws(() => api.future(), /task cleanup failed/);
    });
  },
);

test(
  "pending future/stream reads cancel without faking payloads and await asynchronous host disposal",
  { timeout: 30000 },
  async () => {
    await fixture("values", async (runtime) => {
      let allocator;
      for (const name of ["await", "first"]) {
        const reading = barrier(),
          disposing = barrier(),
          release = barrier();
        let closed = 0;
        const { api, start } = await runtime.instantiate({
          onTaskEvent: (e) => {
            if (e.type === "read-pending") reading.resolve();
          },
          instantiateCore: async (module, imports) => {
            const instance = await WebAssembly.instantiate(module, imports);
            if (instance.exports.alloc) allocator = instance.exports;
            return instance;
          },
        });
        const dispose = async () => {
          closed++;
          disposing.resolve();
          await release.promise;
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
        const task = start(name, [input]);
        const rejected = assert.rejects(
          task.result,
          (e) => e.name === "TaskCancelledError",
        );
        await reading.promise;
        let acknowledged = false;
        const done = task.cancel().then((x) => {
          acknowledged = true;
          return x;
        });
        await disposing.promise;
        assert.equal(
          acknowledged,
          false,
          "disposal acknowledgment must precede task acknowledgment",
        );
        release.resolve();
        assert.equal((await done).status, "cancelled");
        await rejected;
        assert.equal(closed, 1);
        assert.equal(allocator.live(), 0);
        assert.equal(await api.await(Promise.resolve(42n)), 42n);
      }
    });
  },
);
