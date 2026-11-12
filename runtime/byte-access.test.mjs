import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

const formats = [
  ["i16", 2, "Int16"], ["u16", 2, "Uint16"],
  ["i32", 4, "Int32"], ["i64", 8, "BigInt64"],
  ["f32", 4, "Float32"], ["f64", 8, "Float64"],
];
const camel = name => name.replace(/-([a-z0-9])/g, (_, c) => c.toUpperCase());
for (const backend of ["core", "component"])
  test(`${backend}: fixed-width GC byte accesses match DataView and enforce logical bounds`, { timeout: 120000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "walz-byte-access-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/byte-access.walz", directory);
      await instrument(directory, backend);
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      const make = async () => {
        let arena;
        const value = await instantiate({ instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        } });
        return { ...value, clean() {
          assert.equal(arena.live(), 0);
          assert.ok(Object.values(value.__memory()).every(n => n === 0));
        } };
      };
      const { api, clean } = await make();
      assert.equal(api.order(), 12345);
      let seed = 173;
      const next = () => seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      for (const [format, width, dataViewType] of formats) for (const order of ["le", "be"]) {
        const little = order === "le";
        const read = camel(`read-${format}-${order}`), view = camel(`view-${format}-${order}`);
        const write = camel(`write-${format}-${order}`);
        for (let round = 0; round < 64; round++) {
          const bytes = Uint8Array.from({ length: 31 }, () => next() >>> 24);
          const offset = round % (bytes.length - width + 1);
          const expected = new DataView(bytes.buffer)[`get${dataViewType}`](offset, little);
          assert.equal(api[read](bytes, offset), expected, `${read} at ${offset}`);
          // The inner access is relative to a subview starting at an odd address.
          if (offset >= 1 && offset + width <= bytes.length - 1)
            assert.equal(api[view](bytes, 1, bytes.length - 2, offset - 1), expected);
          if (format !== "u16") {
            const result = api[write](bytes, offset, expected);
            if (!Number.isNaN(expected)) assert.deepEqual(result, bytes, `${write} round trip`);
            else assert.ok(Number.isNaN(api[read](result, offset)));
          }
          clean();
        }
        if (format !== "u16") {
          const values = format[0] === "f" ? [0, -0, Infinity, -Infinity, NaN, 1.5, -2.75, 1e-40]
            : format === "i64" ? [0n, -1n, 0x123456789abcdefn, -(1n << 63n), (1n << 63n) - 1n]
              : [0, -1, -2147483648, 2147483647, 65535, 65536, -32769];
          for (const value of values) {
            const bytes = new Uint8Array(width + 4).fill(173), expected = bytes.slice();
            new DataView(expected.buffer)[`set${dataViewType}`](1, value, little);
            const result = api[write](bytes, 1, value);
            // NaN payloads are not fixed by the JS numeric boundary.
            if (!Number.isNaN(value)) assert.deepEqual(result, expected, write);
            else assert.ok(Number.isNaN(new DataView(result.buffer, result.byteOffset)[`get${dataViewType}`](1, little)));
            assert.deepEqual(bytes, new Uint8Array(width + 4).fill(173), "input remains immutable");
            clean();
          }
        }
        // Isolate each trapping call because the public adapter poisons the instance.
        const bytes = new Uint8Array(width + 4);
        const invalid = [
          [read, [bytes, -1]], [read, [bytes, bytes.length]],
          [read, [bytes, bytes.length - width + 1]], [read, [bytes, 0x7fffffff]],
          [read, [new Uint8Array(width - 1), 0]],
          [view, [bytes, 1, width - 1, 0]], // underlying owner has enough bytes
          [view, [bytes, 1, width, 1]], [view, [bytes, 1, width, -1]],
        ];
        if (format !== "u16") for (const offset of [-1, bytes.length, bytes.length - width + 1, 0x7fffffff])
          invalid.push([write, [bytes, offset, format === "i64" ? 1n : 1]]);
        for (const [method, args] of invalid) {
          const bad = await make();
          assert.throws(() => bad.api[method](...args), WebAssembly.RuntimeError, method);
          bad.clean();
        }
      }
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
