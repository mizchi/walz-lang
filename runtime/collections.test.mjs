import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

for (const backend of ["core", "component"])
  test(`${backend}: generic collections match Map and preserve immutable snapshots`, { timeout: 60000 }, async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-collections-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/collections.walz", dir);
      await instrument(dir, backend);
      const { instantiate } = await import(pathToFileURL(join(dir, "index.js")));
      let memory;
      const { api, __memory } = await instantiate({ instantiateCore: async (m, imports) => {
        const instance = await WebAssembly.instantiate(m, imports);
        if (instance.exports.live) memory = instance.exports;
        return instance;
      } });
      const clean = () => { assert.equal(memory.live(), 0); assert.ok(Object.values(__memory()).every(n => n === 0)); };
      let seed = 137;
      const next = () => (seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0);
      for (let round = 0; round < 12; round++) {
        const keys = Array.from({ length: round ? 512 : 0 }, (_, i) => `${i % 3 ? "日本語🦀" : "\0"}${next() % (round % 2 ? 257 : 64)}`);
        const values = keys.map(() => BigInt(next()) - 2147483648n);
        if (values.length) { values[0] = -9223372036854775808n; values[values.length - 1] = 9223372036854775807n; }
        const remove = keys.filter((_, i) => i % 7 === 0).concat(["missing"]);
        const oracle = new Map(keys.map((k, i) => [k, values[i]]));
        remove.forEach(k => oracle.delete(k));
        assert.deepEqual(api.mutations(keys, values, remove, 0), keys.map(k => oracle.get(k)));
        clean();
        assert.deepEqual(api.mutations(keys, values, [], 1), keys.map(() => undefined));
        clean();
      }
      for (const input of [[], ["", "日本語🦀", "\0"], Array.from({ length: 1024 }, (_, i) => String(i))]) {
        assert.deepEqual(api.snapshots(input), [input, ["later"]]); clean();
      }
      const snapshotKeys = ["a", "q", "a", "日本語🦀", "\0"];
      const snapshotValues = [1n, 2n, 3n, 4n, 5n];
      const expectedEntries = [...new Map(snapshotKeys.map((k, i) => [k, snapshotValues[i]]))].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0);
      assert.deepEqual(api.entries([], []), []); clean();
      assert.deepEqual(api.entries(snapshotKeys, snapshotValues).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0), expectedEntries); clean();
      assert.throws(() => api.mutations(["key"], [], [], 0)); clean();
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
