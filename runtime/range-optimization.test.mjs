import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

const sum = bytes => {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let result = 0n;
  for (let i = 0; i + 4 <= bytes.length; i += 4) result += BigInt(view.getInt32(i, true));
  return result;
};
for (const backend of ["core", "component", "native-utf8", "native-js-strings"])
  test(`${backend}: proven view reads preserve values, logical bounds and cleanup`, { timeout: 120000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "walz-range-optimization-"));
    try {
      if (backend.startsWith("native-")) {
        const { build } = await import("./build-js-native.mjs");
        await build("tests/component-fixtures/range-optimization.walz", directory, { mode: backend.slice(7) });
      } else {
        const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
        await build("tests/component-fixtures/range-optimization.walz", directory);
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
            assert.equal(value.__memory().hostHandles, 0);
            assert.equal(value.__memory().boundaryBuffers, 0);
          } else assert.ok(Object.values(value.__memory()).every(n => n === 0));
        } };
      };
      const { api, clean } = await make();
      let seed = 173;
      const next = () => seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      for (const n of [0, 1, 2, 3, 4, 5, 15, 16, 17, 255, 256, 257, 65537]) {
        const bytes = Uint8Array.from({ length: n }, () => next() >>> 24);
        for (let round = 0; round < 60; round++) {
          const p = next() % (n + 1), length = next() % (n - p + 1);
          const q = next() % (length + 1), m = next() % (length - q + 1);
          assert.equal(api.scan(bytes, p, length), sum(bytes.subarray(p, p + length)));
          assert.equal(api.repeat(bytes, p, length), 6n * sum(bytes.subarray(p, p + length)));
          assert.equal(api.nested(bytes, p, length, q, m), sum(bytes.subarray(p + q, p + q + m)));
          if (n >= 4) {
            const at = next() % (n - 3), view = new DataView(bytes.buffer);
            assert.equal(api.twice(bytes, at), (view.getInt32(at, true) + view.getInt32(at, false)) | 0);
          }
          clean();
        }
      }
      for (const [method, args] of [
        ["scan", [new Uint8Array(32), -1, 4]], ["scan", [new Uint8Array(32), 30, 4]],
        ["scan", [new Uint8Array(32), 0, -1]], ["scan", [new Uint8Array(32), 2147483647, 4]],
        ["nested", [new Uint8Array(32), 4, 4, 1, 4]], // Fits the owner, not the outer view.
        ["nested", [new Uint8Array(32), 4, 4, -1, 1]],
        ["twice", [new Uint8Array(32), -1]], ["twice", [new Uint8Array(32), 29]],
      ]) {
        const bad = await make();
        assert.throws(() => bad.api[method](...args), WebAssembly.RuntimeError, method);
        bad.clean();
      }
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
