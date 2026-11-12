import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

for (const backend of ["core", "component", "native-utf8", "native-js-strings"])
  test(`${backend}: scalar values preserve discriminants, identity, handlers and ABI cleanup`, { timeout: 120000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "walz-value-optimization-"));
    try {
      if (backend.startsWith("native-")) {
        const { build } = await import("./build-js-native.mjs");
        await build("tests/component-fixtures/value-optimization.walz", directory, { mode: backend.slice(7) });
      } else {
        const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
        await build("tests/component-fixtures/value-optimization.walz", directory);
        await instrument(directory, backend);
      }
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      const make = async () => {
        let arena;
        const value = await instantiate({ instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        } });
        return { ...value, clean() {
          if (arena) assert.equal(arena.live(), 0);
          if (backend.startsWith("native-")) {
            const memory = value.__memory();
            assert.equal(memory.hostHandles, 0); assert.equal(memory.boundaryBuffers, 0);
          } else assert.ok(Object.values(value.__memory()).every(n => n === 0));
        } };
      };
      const { api, clean } = await make();
      let seed = 29;
      const next = () => seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      const edge = [0, 1, 2, 65535, 65536, 0x7fffffff, 0x80000000, 0xfffffffe, 0xffffffff];
      for (let round = 0; round < 2000; round++) {
        const a = round < edge.length ? edge[round] : next(), b = next(), h = next(), flag = round % 3 - 1;
        const total = BigInt(a) * BigInt(b) + BigInt(h);
        assert.equal(api.layout(a | 0, b | 0, h | 0), total > 0xffffffffn ? -1 : Number(BigInt.asIntN(32, total)));
        assert.equal(api.branch(flag, a | 0), flag ? (2 * a + 1) | 0 : 1);
        assert.equal(api.converted(flag, a | 0), flag ? a | 0 : -7);
        assert.deepEqual(api.escaped(a | 0), { tag: "ok", val: a | 0 });
        assert.equal(api.unit(flag), flag ? 20 : 10);
        assert.equal(api.null(flag), flag ? 21 : 10);
        clean();
      }
      assert.equal(api.identity(), 43);
      assert.equal(api.caught(0), 42); assert.equal(api.caught(1), -7);
      assert.equal(api.handler(), 7); assert.equal(api.ignored(1), 42);
      assert.equal(api.main(), 42); clean();
      const bad = await make();
      assert.throws(() => bad.api.ignored(0), WebAssembly.RuntimeError); bad.clean();
    } finally { await rm(directory, { recursive: true, force: true }); }
  });

for (const backend of ["core", "component"])
  test(`${backend}: a discarded suspended payload still runs and cleans up on rejection/cancel`, { timeout: 60000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "walz-value-optimization-async-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/value-optimization-async.walz", directory);
      await instrument(directory, backend);
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      let calls = 0, disposed = 0, arena;
      const entered = Promise.withResolvers();
      const options = { eventLoop: { sleep(ms) {
        calls++;
        if (ms === 0) return Promise.resolve();
        if (ms === 1) return Promise.reject(new Error("payload rejected"));
        const done = Promise.withResolvers();
        done.promise[Symbol.asyncDispose] = async () => {
          await new Promise(resolve => setTimeout(resolve, 5));
          disposed++; done.resolve();
        };
        entered.resolve(); return done.promise;
      } }, instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } };
      let value = await instantiate(options);
      const clean = () => {
        assert.equal(arena.live(), 0);
        assert.ok(Object.values(value.__memory()).every(n => n === 0));
      };
      const answer = value.api.main(0); assert.ok(answer instanceof Promise);
      assert.equal(await answer, 42n); clean();
      await assert.rejects(value.api.main(1), /payload rejected/); clean();
      // Rejection may poison an instance; cancellation needs an independent one.
      value = await instantiate(options);
      const task = value.start("main", [2]);
      const rejected = assert.rejects(task.result, e => e.name === "TaskCancelledError");
      await entered.promise; await task.cancel(); await rejected;
      assert.equal(calls, 3); assert.equal(disposed, 1); clean();
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
