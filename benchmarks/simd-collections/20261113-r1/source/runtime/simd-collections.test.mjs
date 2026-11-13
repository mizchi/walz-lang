import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { collect, instrument } from "./testing/memory.mjs";

const exec = promisify(callback);
const root = new URL("..", import.meta.url);
const sizes = [0, 1, 2, 3, 4, 5, 7, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 1023, 4099];
const popcount = x => { let count = 0; for (x >>>= 0; x; x = (x & (x - 1)) >>> 0) count++; return count; };
function random(seed = 42) { return () => seed = (Math.imul(seed, 1664525) + 1013904223) | 0; }
function canonical(n, words) {
  const result = Int32Array.from(words);
  if (n % 32) result[result.length - 1] &= 2 ** (n % 32) - 1;
  return result;
}
const indices = (n, words) => Array.from({ length: n }, (_, i) => i).filter(i => (words[i >>> 5] >>> (i & 31)) & 1);
const exactSum = xs => Array.from(xs).reduce((sum, value) => sum + BigInt(value), 0n);
function input(api, values) {
  const object = api.input(values.length);
  for (let i = 0; i < values.length; i++) api.set(object, i, values[i]);
  return object;
}
function words(api, bitmap) {
  return Int32Array.from({ length: api["bitmap-word-len"](bitmap) }, (_, i) => api["bitmap-word"](bitmap, i));
}
async function resident() {
  const directory = await mkdtemp(join(tmpdir(), "walz-simd-collections-"));
  try {
    const wasm = join(directory, "resident.wasm");
    await exec("cargo", ["run", "--locked", "--quiet", "--", "build", "examples/simd-collections/resident.walz", wasm], { cwd: root });
    return (await WebAssembly.instantiate(await readFile(wasm))).instance.exports;
  } finally { await rm(directory, { recursive: true, force: true }); }
}

test("resident bitmaps and columns agree with independent scalar oracles at lane, word and vector boundaries", async () => {
  const api = await resident(), next = random();
  for (const n of sizes) {
    const a = Int32Array.from({ length: Math.ceil(n / 32) }, next);
    const b = Int32Array.from(a, next), ca = canonical(n, a), cb = canonical(n, b);
    const x = input(api, a), y = input(api, b), ba = api.bitmap(n, x), bb = api.bitmap(n, y);
    assert.equal(api["bitmap-len"](ba), n);
    assert.deepEqual(words(api, ba), ca);
    assert.equal(api["bitmap-count"](ba), indices(n, ca).length);
    assert.equal(api["bitmap-count"](api["bitmap-full"](n)), n);
    assert.equal(api["bitmap-count"](api["bitmap-empty"](n)), 0);
    for (const [name, operation] of [["and", (x, y) => x & y], ["or", (x, y) => x | y], ["xor", (x, y) => x ^ y], ["and-not", (x, y) => x & ~y]]) {
      const expected = Int32Array.from(ca, (value, i) => operation(value, cb[i]));
      assert.deepEqual(words(api, api[`bitmap-${name}`](ba, bb)), expected, `${name}/${n}`);
    }
    const intersection = Array.from(ca).reduce((sum, value, i) => sum + popcount(value & cb[i]), 0);
    assert.equal(api["bitmap-intersection-count"](ba, bb), intersection);
    assert.equal(api["bitmap-intersects"](ba, bb), Number(intersection !== 0));
    assert.deepEqual(words(api, api["bitmap-not"](ba)), canonical(n, Int32Array.from(ca, value => ~value)));
    if (n) {
      assert.equal(api["bitmap-get"](ba, n - 1), (ca[(n - 1) >>> 5] >>> ((n - 1) & 31)) & 1);
      api.set(x, 0, ~a[0]); assert.deepEqual(words(api, ba), ca, "packed bitmap is a snapshot");
    }

    const values = Int32Array.from({ length: n }, (_, i) => i % 11 === 0 ? 0 : i % 13 === 0 ? -2147483648 : i % 17 === 0 ? 2147483647 : next());
    const source = input(api, values), column = api.column(source);
    assert.equal(api["column-len"](column), n);
    assert.equal(api["column-sum"](column), exactSum(values));
    assert.equal(api["column-sum-where"](column, ba), exactSum(indices(n, ca).map(i => values[i])));
    for (const needle of [0, -2147483648, 2147483647, 42]) {
      const selected = api["column-eq"](column, needle);
      const expected = Array.from(values).flatMap((value, i) => value === needle ? [i] : []);
      assert.deepEqual(indices(n, words(api, selected)), expected);
      assert.equal(api["column-count-eq"](column, needle), expected.length);
    }
    for (const [lo, hi] of [[-1, 1], [-2147483648, 2147483647], [0, 0], [2, -2], [-2147483648, -2147483647]]) {
      const expected = Array.from(values).flatMap((value, i) => value >= lo && value < hi ? [i] : []);
      assert.deepEqual(indices(n, words(api, api["column-between"](column, lo, hi))), expected);
      assert.equal(api["column-count-between"](column, lo, hi), expected.length);
    }
    if (n) { api.set(source, 0, 123); assert.equal(api["column-get"](column, 0), values[0]); }
    assert.throws(() => api["column-get"](column, n), WebAssembly.RuntimeError);
    assert.throws(() => api["bitmap-get"](ba, n), WebAssembly.RuntimeError);
    assert.throws(() => api["bitmap-word"](ba, Math.ceil(n / 32)), WebAssembly.RuntimeError);
  }
  const zero = api["bitmap-empty"](0), one = api["bitmap-empty"](1);
  for (const name of ["and", "or", "xor", "and-not", "intersects", "intersection-count"])
    assert.throws(() => api[`bitmap-${name}`](zero, one), WebAssembly.RuntimeError);
  assert.throws(() => api["bitmap-empty"](-1), WebAssembly.RuntimeError);
  assert.throws(() => api["bitmap-full"](-1), WebAssembly.RuntimeError);
  assert.throws(() => api.bitmap(33, input(api, [1])), WebAssembly.RuntimeError);
  assert.throws(() => api["column-sum-where"](api.column(input(api, [1])), zero), WebAssembly.RuntimeError);
});

test("resident snapshots and selections release their inputs and temporary owners", { skip: typeof global.gc !== "function" }, async () => {
  const api = await resident();
  const history = (() => {
    const source = input(api, new Int32Array(129).fill(42)), column = api.column(source);
    const selected = api["column-eq"](column, 42), copy = api["bitmap-not"](selected);
    return { refs: [new WeakRef(source), new WeakRef(column), new WeakRef(selected)], copy };
  })();
  for (let pass = 0; pass < 8; pass++) {
    await collect();
    if (history.refs.every(ref => ref.deref() === undefined)) break;
  }
  assert.ok(history.refs.every(ref => ref.deref() === undefined));
  assert.equal(api["bitmap-len"](history.copy), 129);
  assert.equal(api["bitmap-count"](history.copy), 0);
  const kept = api.column(input(api, [1, 2, 3])), ref = new WeakRef(kept);
  await collect(); assert.ok(ref.deref()); assert.equal(api["column-sum"](kept), 6n);
});

for (const backend of ["core", "component"]) test(`${backend}: SIMD collections compose through the value ABI and reclaim buffers after results and traps`, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-simd-collections-abi-"));
  try {
    const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
    await build("examples/simd-collections/portable.walz", directory);
    await instrument(directory, backend);
    const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
    async function create() {
      let arena;
      const instance = await instantiate({ instantiateCore: async (module, imports) => {
        const value = await WebAssembly.instantiate(module, imports);
        if (value.exports.live) arena = value.exports;
        return value;
      } });
      return { api: instance.api, clean() { assert.equal(arena.live(), 0); assert.ok(Object.values(instance.__memory()).every(n => n === 0)); } };
    }
    const { api, clean } = await create(), next = random(123);
    for (const n of sizes) {
      const a = Int32Array.from({ length: Math.ceil(n / 32) }, next), b = Int32Array.from(a, next);
      const ca = canonical(n, a), cb = canonical(n, b);
      for (const [op, f] of [[0, (x, y) => x & y], [1, (x, y) => x | y], [2, (x, y) => x ^ y], [3, (x, y) => x & ~y]])
        assert.deepEqual(Int32Array.from(api.combine(n, a, b, op)), Int32Array.from(ca, (v, i) => f(v, cb[i])));
      assert.deepEqual(Int32Array.from(api.complement(n, a)), canonical(n, Int32Array.from(ca, v => ~v)));
      assert.deepEqual(Array.from(api.indices(n, a)), indices(n, ca));
      assert.equal(api.count(n, a), indices(n, ca).length);
      assert.equal(api.intersectionCount(n, a, b), Array.from(ca).reduce((sum, v, i) => sum + popcount(v & cb[i]), 0));
      const values = Int32Array.from({ length: n }, (_, i) => i % 5 ? i - 127 : 0);
      assert.deepEqual(Int32Array.from(api.roundtrip(values)), values);
      assert.equal(api.sum(values), exactSum(values));
      assert.equal(api.sumWhere(values, a), exactSum(indices(n, ca).map(i => values[i])));
      assert.deepEqual(Array.from(api.equal(values, 0)), Array.from(values).flatMap((v, i) => v === 0 ? [i] : []));
      assert.equal(api.countEq(values, 0), api.equal(values, 0).length);
      assert.equal(api.countBetween(values, -42, 42), api.between(values, -42, 42).length);
      clean();
    }
    const answer = api.query([10, 20, 22, 70], [1, 2, 2, 1], 10, 30, 2);
    assert.equal(answer.count, 2); assert.equal(answer.sum, 42n); assert.deepEqual(Array.from(answer.indices), [1, 2]); clean();
    for (const failure of [
      api => api.count(-1, []), api => api.count(33, [1]), api => api.combine(1, [1], [1], 4),
      api => api.get([42], -1), api => api.get([42], 1), api => api.sumWhere([1], []),
      api => api.query([1], [], 0, 2, 0),
    ]) { const value = await create(); assert.throws(() => failure(value.api), WebAssembly.RuntimeError); value.clean(); }
  } finally { await rm(directory, { recursive: true, force: true }); }
});
