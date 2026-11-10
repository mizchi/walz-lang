import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "./build.mjs";
import { instrument, tick, collect } from "./testing/memory.mjs";
const empty = {
  bufferRegistry: 0,
  scopes: 0,
  tasks: 0,
  futures: 0,
  streams: 0,
  futureEnds: 0,
  streamEnds: 0,
  handles: 0,
  threads: 0,
  subtasks: 0,
};
test(
  "finished, closed and transferred endpoints leave no live jco ownership entries",
  { timeout: 60000 },
  async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-memory-"));
    try {
      await build("tests/component-fixtures/values.walz", dir);
      await instrument(dir);
      const runtime = await import(pathToFileURL(join(dir, "index.js")));
      const instance = await runtime.instantiate();
      const { api, start, __memory } = instance;
      const operations = {
        future: async () => assert.equal(await api.future(), 42n),
        stream: async () => {
          const items = [];
          for await (const n of api.stream()) items.push(n);
          assert.deepEqual(items, [40n, 2n]);
        },
        await: async () =>
          assert.equal(await api.await(Promise.resolve(42n)), 42n),
        first: async () => assert.equal(await api.first([42n]), 42n),
        returnedFuture: async () =>
          assert.equal(await api.await(api.future()), 42n),
        returnedStream: async () =>
          assert.equal(await api.first(api.stream()), 40n),
        earlyReturn: async () => {
          const s = api.stream();
          await s.return();
          await s.return();
        },
        close: () => start("closeStream", [[42n]]).result,
      };
      for (const [name, run] of Object.entries(operations)) {
        for (let n = 0; n < 10; n++) await run();
        await tick();
        assert.deepEqual(__memory(), empty, name);
      }
      for (const name of ["await", "first"]) {
        let onRead;
        const { start: cancelStart, __memory: cancelMemory } =
          await runtime.instantiate({
            onTaskEvent: (e) => {
              if (e.type === "read-pending") onRead?.();
            },
          });
        for (let i = 0; i < 20; i++) {
          const reading = new Promise((r) => {
            onRead = r;
          });
          let release;
          const pending = new Promise((r) => {
            release = r;
          });
          const input =
            name === "await"
              ? Object.assign(pending, {
                  [Symbol.asyncDispose]() {
                    release(42n);
                  },
                })
              : {
                  [Symbol.asyncIterator]() {
                    return this;
                  },
                  next() {
                    return pending;
                  },
                  return() {
                    release({ done: true });
                    return { done: true };
                  },
                };
          const task = cancelStart(name, [input]);
          const rejected = assert.rejects(
            task.result,
            (e) => e.name === "TaskCancelledError",
          );
          await reading;
          assert.equal((await task.cancel()).status, "cancelled");
          await rejected;
          await tick();
          assert.deepEqual(cancelMemory(), empty, `cancel ${name}`);
        }
      }
      const held = api.future();
      assert.ok(
        __memory().futures > 0,
        "an unread returned endpoint still belongs to the caller",
      );
      assert.equal(await held, 42n);
      await tick();
      assert.deepEqual(__memory(), empty);
      if (typeof global.gc === "function") {
        const discarded = async (trap) => {
          let memory;
          const { api: temporary } = await runtime.instantiate({
            instantiateCore: async (m, i) => {
              const result = await WebAssembly.instantiate(m, i);
              if (result.exports.alloc) memory = result.exports.memory;
              return result;
            },
          });
          if (trap)
            assert.throws(() => temporary.units(["invalid", undefined]));
          else assert.equal(temporary.value(undefined), undefined);
          return new WeakRef(memory);
        };
        for (const trap of [false, true]) {
          const ref = await discarded(trap);
          await collect();
          assert.ok(
            ref.deref() === undefined,
            `discarded ${trap ? "trapped" : "normal"} stores must be collectible`,
          );
        }
      }
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  },
);

test(
  "varying bytes sizes reuse freed linear storage",
  { timeout: 60000 },
  async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-memory-sizes-"));
    try {
      await build("tests/component-fixtures/buffers.walz", dir);
      let allocator;
      const { instantiate } = await import(
        pathToFileURL(join(dir, "index.js"))
      );
      const { api } = await instantiate({
        instantiateCore: async (m, i) => {
          const instance = await WebAssembly.instantiate(m, i);
          if (instance.exports.alloc) allocator = instance.exports;
          return instance;
        },
      });
      for (let size = 1; size <= 20000; size++) {
        assert.equal(api.bytes(new Uint8Array(size)).length, size);
        assert.equal(allocator.live(), 0);
      }
      assert.ok(
        allocator.memory.buffer.byteLength < 1024 * 1024,
        "a 20 KB value should not exhaust a 16 MB arena",
      );
      let seed = 42;
      for (let round = 0; round < 100; round++) {
        const blocks = [];
        for (let i = 0; i < 32; i++) {
          seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
          const size = 1 + (seed % 8192),
            ptr = allocator.alloc(size);
          new Uint8Array(allocator.memory.buffer, ptr, size).fill(i + 1);
          blocks.push({ size, ptr, value: i + 1 });
        }
        while (blocks.length) {
          seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
          const [block] = blocks.splice(seed % blocks.length, 1);
          assert.ok(
            new Uint8Array(
              allocator.memory.buffer,
              block.ptr,
              block.size,
            ).every((x) => x === block.value),
            "live allocations must not overlap",
          );
          allocator.free(block.ptr);
        }
        assert.equal(allocator.live(), 0);
      }
      assert.ok(allocator.memory.buffer.byteLength < 1024 * 1024);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  },
);
