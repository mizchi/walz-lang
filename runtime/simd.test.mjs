import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

for (const backend of ["core", "component"])
  test(`${backend}: SIMD uses portable scalar/list boundaries and survives JSPI`, { timeout: 60000 }, async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-simd-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/simd.walz", dir);
      await instrument(dir, backend);
      const { instantiate } = await import(pathToFileURL(join(dir, "index.js")));
      let memory, yields = 0;
      const { api, __memory } = await instantiate({
        eventLoop: { yield: async () => { yields++; await new Promise(resolve => setTimeout(resolve, 1)); } },
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.live) memory = instance.exports;
          return instance;
        },
      });
      const clean = () => { assert.equal(memory.live(), 0); assert.ok(Object.values(__memory()).every(v => v === 0)); };
      let seed = 2026;
      const next = () => (seed = (Math.imul(seed, 1664525) + 1013904223) | 0);
      for (let i = 0; i < 64; i++) {
        const a = Array.from({ length: 4 }, next), b = Array.from({ length: 4 }, next);
        assert.equal(api.add(a[0], b[0]), (a[0] + b[0]) | 0);
        assert.equal(api.dot(a, b), a.reduce((sum, x, j) => (sum + Math.imul(x, b[j])) | 0, 0));
        assert.deepEqual(api.shuffle(a, b), new Int32Array([b[0], a[0], b[1], a[1]]));
        clean();
      }
      assert.equal(await api.suspended(-2147483648), -2147483648);
      assert.equal(yields, 1); clean();
      assert.equal(await api.ready(), 42); clean();
      assert.throws(() => api.dot([], [1, 2, 3, 4])); clean();
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
