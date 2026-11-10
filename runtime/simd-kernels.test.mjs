import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";
import { instrument } from "./testing/memory.mjs";

const execFile = promisify(execFileCallback);
const root = new URL("..", import.meta.url);
const popcnt = x => { x >>>= 0; let n = 0; while (x) { x &= x - 1; n++; } return n; };
const dot = (a, b) => a.reduce((n, x, i) => n + x * b[i], 0);
const close = (actual, a, b) => {
  const expected = dot(a, b), scale = a.reduce((n, x, i) => n + Math.abs(x * b[i]), 0);
  assert.ok(Math.abs(actual - expected) <= Math.max(1e-6, scale * 2e-6), `${actual} != ${expected}`);
};

test("jsimd-style GC kernels handle tails, wide sums, snapshots and invalid lengths", async () => {
  const dir = await mkdtemp(join(tmpdir(), "walz-kernels-core-"));
  try {
    const path = join(dir, "kernels.wasm");
    await execFile("cargo", ["run", "--locked", "--quiet", "--", "build", "examples/simd-kernels/resident.walz", path], { cwd: root });
    const { instance: { exports: api } } = await WebAssembly.instantiate(await readFile(path));
    const load = (kind, xs) => {
      const value = api[`${kind}-new`](xs.length);
      for (let i = 0; i < xs.length; i++) api[`${kind}-set`](value, i, xs[i]);
      return value;
    };
    let seed = 42;
    const next = () => (seed = (Math.imul(seed, 1664525) + 1013904223) | 0);
    for (const n of [0, 1, 2, 3, 4, 5, 7, 15, 16, 17, 31, 32, 33, 257, 4099]) {
      const a = Int32Array.from({ length: n }, next), b = Int32Array.from({ length: n }, next);
      const x = load("i32", a), y = load("i32", b), px = api["pack-i32"](x), py = api["pack-i32"](y);
      const sum = Array.from(a).reduce((n, x) => n + BigInt(x), 0n);
      const intersection = Array.from(a).reduce((n, x, i) => n + popcnt(x & b[i]), 0);
      for (const [name, args, expected] of [
        ["sum-scalar", [x], sum], ["sum-simd", [px], sum],
        ["bitmap-scalar", [x, y], intersection], ["bitmap-simd", [px, py], intersection],
      ]) assert.equal(api[name](...args), expected, `${name} length=${n}`);
      if (n) { api["i32-set"](x, 0, a[0] ^ -1); assert.equal(api["sum-simd"](px), sum); }
      const fa = Float32Array.from({ length: n }, () => Math.fround(next() / 2 ** 28));
      const fb = Float32Array.from({ length: n }, () => Math.fround(next() / 2 ** 28));
      const fx = load("f32", fa), fy = load("f32", fb);
      close(api["dot-scalar"](fx, fy), fa, fb);
      close(api["dot-simd"](api["pack-f32"](fx), api["pack-f32"](fy)), fa, fb);
      const bytes = Uint8Array.from({ length: n }, (_, i) => i % 251 + 1), raw = load("i32", bytes);
      for (const needle of [0, 1, 16, 17, 32, 33, 127, 251, 255]) {
        assert.equal(api["bytes-scalar"](raw, needle), bytes.indexOf(needle));
        assert.equal(api["bytes-simd"](api["pack-bytes"](raw), needle), bytes.indexOf(needle));
      }
    }
    const ones = load("i32", [2147483647, 2147483647, -2147483648, -2147483648, 7]);
    assert.equal(api["sum-simd"](api["pack-i32"](ones)), 5n);
    assert.throws(() => api["i32-new"](-1), WebAssembly.RuntimeError);
    assert.throws(() => api["bitmap-simd"](api["pack-i32"](ones), api["pack-i32"](load("i32", []))), WebAssembly.RuntimeError);
    assert.throws(() => api["dot-simd"](api["pack-f32"](load("f32", [1])), api["pack-f32"](load("f32", []))), WebAssembly.RuntimeError);
    assert.throws(() => api["bytes-simd"](api["pack-bytes"](load("i32", [])), 256), WebAssembly.RuntimeError);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

for (const backend of ["core", "component"])
  test(`${backend}: jsimd-style kernels include portable copies and reclaim ABI allocations`, async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-kernels-abi-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("examples/simd-kernels/portable.walz", dir);
      await instrument(dir, backend);
      const { instantiate } = await import(pathToFileURL(join(dir, "index.js")));
      const create = async () => {
        let arena;
        const { api, __memory } = await instantiate({ instantiateCore: async (m, i) => {
          const instance = await WebAssembly.instantiate(m, i);
          if (instance.exports.live) arena = instance.exports;
          return instance;
        } });
        return { api, clean: () => { assert.equal(arena.live(), 0); assert.ok(Object.values(__memory()).every(x => x === 0)); } };
      };
      const { api, clean } = await create();
      for (const suffix of ["scalar", "simd"]) {
        assert.equal(api[`sum${suffix === "simd" ? "Simd" : "Scalar"}`]([2147483647, 2147483647, 42]), 4294967336n);
        assert.equal(api[`dot${suffix === "simd" ? "Simd" : "Scalar"}`]([1, 2, 3, 4, 5], [2, 4, 6, 8, 10]), 110);
        assert.equal(api[`bitmap${suffix === "simd" ? "Simd" : "Scalar"}`]([-1, 15, 0, 8, 1], [3, 7, 9, 8, 1]), 7);
        for (const n of [0, 1, 15, 16, 17, 257]) {
          const bytes = Uint8Array.from({ length: n }, (_, i) => i % 7 + 1);
          for (const needle of [0, 1, 7, 255])
            assert.equal(api[`index${suffix === "simd" ? "Simd" : "Scalar"}`](bytes, needle), bytes.indexOf(needle));
        }
        clean();
      }
      // A Wasm trap poisons a component instance. Test each failure on its own
      // instance so a previous trap cannot masquerade as another validation.
      for (const fail of [
        api => api.dotSimd([1], []),
        api => api.bitmapSimd([1], []),
        api => api.indexSimd(new Uint8Array(), 256),
      ]) {
        const instance = await create();
        assert.throws(() => fail(instance.api), /unreachable/);
        instance.clean();
      }
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
