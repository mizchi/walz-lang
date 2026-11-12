import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const expected = bytes => { try { return decoder.decode(bytes); } catch { return undefined; } };
for (const backend of ["core", "component", "native-utf8", "native-js-strings"])
  test(`${backend}: UTF-8 range validation and views agree with strict TextDecoder`, { timeout: 120000 }, async () => {
    const root = await mkdtemp(join(tmpdir(), "walz-utf8-views-"));
    try {
      if (backend.startsWith("native-")) {
        const { build } = await import("./build-js-native.mjs");
        await build("tests/component-fixtures/utf8-views.walz", root, { mode: backend.slice(7) });
      } else {
        const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
        await build("tests/component-fixtures/utf8-views.walz", root);
        await instrument(root, backend);
      }
      const { instantiate } = await import(pathToFileURL(join(root, "index.js")));
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
      assert.equal(api.order(), 123456);
      const check = (bytes, p = 0, n = bytes.length) => {
        const text = expected(bytes.subarray(p, p + n)), valid = text !== undefined;
        assert.equal(api.validRange(bytes, p, n), +valid, `range ${p},${n}: ${Array.from(bytes)}`);
        assert.equal(api.validView(bytes, p, n), +valid);
        assert.equal(api.length(bytes, p, n), valid ? n : undefined);
        assert.equal(api.decode(bytes, p, n), text);
        assert.equal(api.materialize(bytes, p, n), text);
        assert.equal(api.text(bytes, p, n, 0, n), text);
        clean();
      };
      const valid = ["", "\0", "ascii", "é日🦀", "\ufeffA\ufeff", "\u007f\u0080\u07ff\u0800\ud7ff\ue000\uffff\u{10000}\u{10ffff}"];
      for (const text of valid) {
        const part = encoder.encode(text), framed = Uint8Array.of(255, ...part, 255);
        assert.equal(api.valid(part), 1);
        assert.equal(api.valid(framed), 0);
        check(framed, 1, part.length); // invalid data outside the range must be ignored
        check(part, 0, part.length);
        for (let p = 0; p <= part.length; p++) for (let end = p; end <= part.length; end++) check(part, p, end - p);
      }
      for (const part of [
        [128], [191], [192, 128], [193, 191], [194], [194, 65], [224, 128, 128],
        [237, 160, 128], [237, 191, 191], [240, 128, 128, 128], [244, 144, 128, 128],
        [245, 128, 128, 128], [248, 128, 128, 128, 128], [255], [226, 130],
      ]) check(Uint8Array.from(part));
      // Exercise both sides of the validator's eight-byte ASCII fast path.
      for (const prefix of [0, 1, 7, 8, 9, 15, 16, 17, 255, 256]) {
        const framed = Uint8Array.of(255, ...encoder.encode("a".repeat(prefix) + "🦀"), 255);
        for (const extra of [0, 1, 2, 3, 4]) check(framed, 1, prefix + extra);
      }
      let seed = 41;
      const next = () => seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      for (let round = 0; round < 300; round++) {
        const bytes = Uint8Array.from({ length: next() % 65 }, () => next() >>> 24);
        const p = next() % (bytes.length + 1), n = next() % (bytes.length - p + 1);
        assert.equal(api.valid(bytes), +(expected(bytes) !== undefined));
        check(bytes, p, n);
      }
      assert.equal(api.text(encoder.encode("é日🦀"), 0, 9, 2, 3), "日");
      // A successful validation is a snapshot of immutable guest input.
      const bytes = encoder.encode("🦀"), copy = api.decode(bytes, 0, bytes.length);
      bytes.fill(255); assert.equal(copy, "🦀");
      for (const method of ["validRange", "validView", "decode", "materialize", "length", "text"])
        for (const [p, n] of [[-1, 0], [0, -1], [4, 0], [3, 1], [1, 3], [2147483647, 2147483647]]) {
          const bad = await make();
          const args = [Uint8Array.of(65, 66, 67), p, n];
          if (method === "text") args.push(0, 0);
          assert.throws(() => bad.api[method](...args), WebAssembly.RuntimeError);
          bad.clean();
        }
      for (const [start, n] of [[1, 0], [0, 1], [2, 1], [0, 10]]) {
        const bad = await make();
        assert.throws(() => bad.api.text(encoder.encode("é日🦀"), 0, 9, start, n), WebAssembly.RuntimeError);
        bad.clean();
      }
    } finally { await rm(root, { recursive: true, force: true }); }
  });
