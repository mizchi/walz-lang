import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

const littleEndian = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;
const cases = [
  ["ints", Int32Array, "Int32", [-2147483648, -1, 0, 2147483647]],
  ["longs", BigInt64Array, "BigInt64", [-9223372036854775808n, -1n, 0n, 9223372036854775807n]],
  ["floats", Float32Array, "Float32", [0, -0, Math.PI, NaN, Infinity, -Infinity]],
  ["doubles", Float64Array, "Float64", [0, -0, Math.PI, NaN, Infinity, -Infinity]],
];

// Count element DataView operations rather than asserting wall-clock timings.
function counted(method, fn) {
  const names = [`get${method}`, `set${method}`], original = names.map(n => DataView.prototype[n]);
  const counts = [0, 0];
  try {
    names.forEach((name, i) => { DataView.prototype[name] = function (...args) { counts[i]++; return original[i].apply(this, args); }; });
    const value = fn();
    return { value, counts };
  } finally { names.forEach((name, i) => { DataView.prototype[name] = original[i]; }); }
}

for (const backend of ["core", "component"])
  test(`${backend}: numeric lists copy typed views, keep snapshots and validate fallback values`, { timeout: 60000 }, async () => {
    const dir = await mkdtemp(join(tmpdir(), "wlt-numeric-lists-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/numeric-lists.wlt", dir);
      await instrument(dir, backend);
      const { instantiate } = await import(pathToFileURL(join(dir, "index.js")));
      const create = async (factory = instantiate) => {
        let arena;
        const instance = await factory({ instantiateCore: async (m, i) => {
          const instance = await WebAssembly.instantiate(m, i);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        } });
        return { ...instance, arena, clean: () => {
          assert.equal(arena.live(), 0);
          assert.ok(Object.values(instance.__memory()).every(n => n === 0));
        } };
      };
      const instance = await create(instantiate), { api, arena, clean } = instance;
      for (const [name, Type, method, values] of cases) {
        const source = new Type(132);
        for (let i = 0; i < source.length; i++) source[i] = values[i % values.length];
        const input = source.subarray(2, 130), expected = new Type(input);
        const { value: output, counts } = counted(method, () => api[name](input));
        assert.deepEqual(output, expected, name);
        if (backend === "core" && littleEndian) assert.deepEqual(counts, [0, 0], `${name}: numeric elements must be copied in bulk`);
        assert.notEqual(output.buffer, input.buffer);
        assert.notEqual(output.buffer, arena.memory.buffer);
        input.fill(values[2]); assert.deepEqual(output, expected);
        output.fill(values[0]);
        assert.deepEqual(api[name]([]), new Type());
        assert.deepEqual(api[name](values), new Type(values));
        clean();
      }
      const before = arena.memory.buffer.byteLength;
      const large = Int32Array.from({ length: 100000 }, (_, i) => (i * -7919) | 0);
      assert.deepEqual(api.ints(large), large);
      assert.ok(arena.memory.buffer.byteLength > before);
      const saved = api.doubles(new Float64Array([Math.PI, -0, Infinity]));
      api.ints(large); assert.deepEqual(saved, new Float64Array([Math.PI, -0, Infinity])); clean();
      assert.deepEqual(api.nested([new Int32Array([1, 2]), [], new Int32Array([-1])]), [new Int32Array([1, 2]), new Int32Array(), new Int32Array([-1])]);
      assert.deepEqual(await api.later(Promise.resolve(new Int32Array([40, 2]))), new Int32Array([40, 2]));
      assert.deepEqual(await api.first([new Float64Array([Math.PI, -0])]), new Float64Array([Math.PI, -0])); clean();

      if (backend === "core") {
        const floatBits = new Uint32Array([0x80000000, 0x7fc12345, 0xffc54321, 1, 0x7f800000]);
        const floatOut = api.floats(new Float32Array(floatBits.buffer));
        assert.deepEqual(new Uint32Array(floatOut.buffer), floatBits);
        const doubleBits = new BigUint64Array([0x8000000000000000n, 0x7ff8123456789abcn, 0xfff8abcdef123456n, 1n]);
        const doubleOut = api.doubles(new Float64Array(doubleBits.buffer));
        assert.deepEqual(new BigUint64Array(doubleOut.buffer), doubleBits);
        for (const [name, values] of [
          ["ints", [0, 2147483648]], ["ints", [0, 1.5]], ["ints", [0, NaN]],
          ["longs", [0n, 9223372036854775808n]], ["longs", [0n, 1]],
          ["floats", [1, "2"]], ["doubles", [1, undefined]],
        ]) {
          assert.throws(() => api[name](values), /expected/); clean();
        }
        let reads = 0;
        const array = [0]; Object.defineProperty(array, 0, { get() { reads++; return 42; } });
        assert.deepEqual(api.ints(array), new Int32Array([42])); assert.equal(reads, 1);
        const proxy = new Proxy(new Int32Array(1), { get(t, p) { return p === "length" ? 1 : p === "0" ? "invalid" : Reflect.get(t, p, t); } });
        assert.throws(() => api.ints(proxy), /expected s32/); clean();
        const forged = new Uint32Array([4294967295]); Object.setPrototypeOf(forged, Int32Array.prototype);
        assert.throws(() => api.ints(forged), /expected s32/); clean();
        const shortened = new Int32Array([1, 2, 3]); Object.defineProperty(shortened, "length", { value: 2 });
        assert.deepEqual(api.ints(shortened), new Int32Array([1, 2]));
        const detached = new Int32Array(2); detached.buffer.transfer();
        assert.deepEqual(api.ints(detached), new Int32Array()); clean();
        assert.throws(() => api.ints(new Int32Array(4194305)), /bounded list/); clean();

        // Force the endian fallback without requiring a big-endian CI machine.
        const bindings = await readFile(join(dir, "bindings.mjs"), "utf8");
        const marker = "const __wltLE=new Uint8Array(new Uint16Array([1]).buffer)[0]===1;";
        assert.ok(bindings.includes(marker));
        await writeFile(join(dir, "bindings-be.mjs"), bindings.replace(marker, "const __wltLE=false;"));
        const index = await readFile(join(dir, "index.js"), "utf8");
        await writeFile(join(dir, "index-be.js"), index.replace("'./bindings.mjs'", "'./bindings-be.mjs'"));
        const { instantiate: be } = await import(pathToFileURL(join(dir, "index-be.js")));
        const fallback = await create(be);
        for (const [name, Type, method, values] of cases) {
          const input = new Type(values), result = counted(method, () => fallback.api[name](input));
          assert.deepEqual(result.value, input);
          assert.equal(result.counts[0], input.length); assert.equal(result.counts[1], input.length);
        }
        fallback.clean();
      }
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
