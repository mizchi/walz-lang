import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { instrument, tick, collect } from "./testing/memory.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
for (const backend of ["core", "component"])
  test(`${backend}: zero-copy views preserve ranges, UTF-8 and suspended roots`, { timeout: 60000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "walz-views-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/views.walz", directory);
      await instrument(directory, backend);
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      const make = async () => {
        let arena;
        const value = await instantiate({
          instantiateCore: async (module, imports) => {
            const instance = await WebAssembly.instantiate(module, imports);
            if (instance.exports.live) arena = instance.exports;
            return instance;
          },
        });
        return { ...value, clean() {
          assert.equal(arena.live(), 0);
          assert.ok(Object.values(value.__memory()).every(n => n === 0));
        } };
      };
      let resume;
      const { api, clean } = await make();
      let seed = 41;
      const next = n => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed % n; };
      for (let round = 0; round < 60; round++) {
        const data = Uint8Array.from({ length: next(256) }, () => next(256));
        const a = next(data.length + 1), n = next(data.length - a + 1);
        const b = next(n + 1), m = next(n - b + 1);
        assert.deepEqual(api.bytes(data, a, n, b, m), data.slice(a + b, a + b + m));
        if (n) assert.equal(api.byte(data, a, n, n - 1), data[a + n - 1]);
        clean();
      }
      for (const input of ["", "ascii", "é日🦀\0x", "a".repeat(257) + "🦀日本語"]) {
        const data = encoder.encode(input);
        const boundaries = Array.from({ length: data.length + 1 }, (_, i) => i)
          .filter(i => i === data.length || (data[i] & 192) !== 128);
        for (let round = 0; round < 30; round++) {
          const a = next(boundaries.length), z = a + next(boundaries.length - a);
          const b = a + next(z - a + 1), y = b + next(z - b + 1);
          assert.equal(api.text(input, boundaries[a], boundaries[z] - boundaries[a],
            boundaries[b] - boundaries[a], boundaries[y] - boundaries[b]),
          decoder.decode(data.subarray(boundaries[b], boundaries[y])));
          assert.equal(api.length(input, boundaries[a], boundaries[z] - boundaries[a]), 0);
          clean();
        }
      }
      const pause = new Promise(resolve => { resume = resolve; });
      const pending = api.after("a".repeat(65536) + "🦀tail", 65536, 4, pause);
      await tick();
      assert.equal(typeof resume, "function");
      if (global.gc) await collect();
      resume();
      assert.equal(await pending, "🦀");
      clean();

      // A Wasm trap terminates an instance; isolate each invalid input.
      for (const [method, args] of [
        ["bytes", [new Uint8Array([1, 2, 3]), -1, 0, 0, 0]],
        ["bytes", [new Uint8Array([1, 2, 3]), 2, 2, 0, 0]],
        ["bytes", [new Uint8Array([1, 2, 3]), 1, 1, 0, 2]],
        ["bytes", [new Uint8Array([1, 2, 3]), 0, -1, 0, 0]],
        ["bytes", [new Uint8Array([1, 2, 3]), 2147483647, 2147483647, 0, 0]],
        ["byte", [new Uint8Array([1, 2, 3]), 0, 1, 1]],
        ["byte", [new Uint8Array([1, 2, 3]), 0, 1, -1]],
        ["text", ["é🦀日", 1, 0, 0, 0]],
        ["text", ["é🦀日", 0, 1, 0, 0]],
        ["text", ["é🦀日", 2, 4, 1, 0]],
        ["text", ["é🦀日", 2, 4, 0, 3]],
        ["text", ["é🦀日", 2, 4, 0, 7]],
      ]) {
        const bad = await make();
        assert.throws(() => bad.api[method](...args), undefined, `${method}: ${args}`);
        bad.clean();
      }
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
