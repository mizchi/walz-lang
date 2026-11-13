import test, { before, after } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "./table/build.mjs";
import { instantiate } from "./table/index.mjs";

let directory, temporary;
before(async () => {
  directory = process.env.WALZ_TABLE_KERNEL ? resolve(process.env.WALZ_TABLE_KERNEL) : await mkdtemp(join(tmpdir(), "walz-table-transfer-"));
  temporary = !process.env.WALZ_TABLE_KERNEL; if (temporary) await build(directory);
});
after(async () => { if (temporary) await rm(directory, { recursive: true, force: true }); });

test("numeric transfer has checked borrowing, snapshot isolation and trap cleanup", async () => {
  const api = (await WebAssembly.instantiate(await readFile(join(directory, "core.wasm")))).instance.exports;
  assert.equal(typeof api["transfer-open"], "function");
  assert.equal(api["transfer-live"](), 0);
  api["transfer-open"](16); assert.throws(() => api["transfer-open"](16), WebAssembly.RuntimeError);
  api["transfer-close"](); assert.equal(api["transfer-live"](), 0);
  assert.throws(() => api["transfer-open"](-1), WebAssembly.RuntimeError); assert.equal(api["transfer-live"](), 0);
  assert.throws(() => api["transfer-open"](589825), WebAssembly.RuntimeError); assert.equal(api["transfer-live"](), 0);
  const text = api.text(api["bytes-new"](0));
  assert.throws(() => api["column-import"](text, 1, text), WebAssembly.RuntimeError);
  api["transfer-open"](16);
  new Uint8Array(api.memory.buffer)[16] = 99;
  try { assert.throws(() => api["column-import"](text, 1, text), WebAssembly.RuntimeError); }
  finally { api["transfer-close"](); }
  assert.equal(api["transfer-live"](), 0);
  api["transfer-open"](16); new Uint8Array(api.memory.buffer)[16] = 2;
  new Float64Array(api.memory.buffer, 24, 1)[0] = NaN;
  try { assert.throws(() => api["column-import"](text, 1, text), WebAssembly.RuntimeError); }
  finally { api["transfer-close"](); }
  api["transfer-open"](16); new Uint8Array(api.memory.buffer)[16] = 3;
  new Float64Array(api.memory.buffer, 24, 1)[0] = 42;
  const c = api["column-import"](text, 1, text); api["transfer-close"]();
  new Float64Array(api.memory.buffer, 24, 1)[0] = 99;
  const cols = api["columns-new"](1); api["column-set"](cols, 0, c);
  assert.equal(api["cell-number"](api.get(api["table-new"](cols, 1), 0, 0)), 42);
});

test("bulk facade preserves scalar tags, special names and input/output snapshots", async () => {
  const base = pathToFileURL(directory + "/");
  await assert.rejects(instantiate(base, { io: "invalid" }), /io/i);
  const data = { n: [undefined, null, NaN, -0, 0, Infinity, -Infinity, Number.MIN_VALUE, 2 ** 53],
    b: [null, undefined, true, false, true, false, null, undefined, true], s: ["🦀", null, undefined, "", "\ufeff", "\0", "a", "é", "x"] };
  Object.defineProperty(data, "__proto__", { enumerable: true, value: data.n });
  const a = await instantiate(base, { io: "bulk" }), b = await instantiate(base, { io: "scalar" });
  const source = a.table(data), expected = b.table(data);
  assert.deepEqual(source.objects(), expected.objects());
  assert.equal(Object.getPrototypeOf(source.objects()[0]), Object.prototype);
  assert.ok(Object.hasOwn(source.objects()[0], "__proto__"));
  assert.deepEqual(source.orderby("n").slice(1).filter(d => !(d.n < 0)).objects(), expected.orderby("n").slice(1).filter(d => !(d.n < 0)).objects());
  const output = source.array("n"); output[3] = 99; data.n[3] = 100;
  const large = a.table({ larger: Float64Array.from({ length: 65537 }, (_, i) => i) });
  assert.equal(large.array("larger")[65536], 65536);
  assert.ok(Object.is(source.get("n", 3), -0));
  assert.deepEqual(source.select().objects(), Array.from({ length: 9 }, () => ({})));
  const lateString = Array(65537).fill(null); lateString[65536] = "late🦀";
  const promoted = a.table({ value: lateString });
  assert.deepEqual(promoted.slice(65534).array("value"), [null, null, "late🦀"]);
  let reads = 0; const reentrant = [0, 2, 3];
  Object.defineProperty(reentrant, 0, { get() { reads++; return a.table({ x: [7] }).get("x"); } });
  assert.deepEqual(a.table({ value: reentrant }).array("value"), [7, 2, 3]);
  assert.equal(reads, 1, "input getters run once and outside the scratch borrow");
});

test("bulk transfer boundary calls scale with columns, not numeric cells", async () => {
  const original = WebAssembly.instantiate, counts = new Map(); let aq, api;
  try {
    WebAssembly.instantiate = async (...args) => {
      const value = await original(...args); api = value.instance.exports;
      const exports = Object.fromEntries(Object.entries(api).map(([name, fn]) => [name, typeof fn !== "function" ? fn : (...args) => {
        counts.set(name, (counts.get(name) ?? 0) + 1); return fn(...args);
      }]));
      return { instance: { exports }, module: value.module };
    };
    aq = await instantiate(pathToFileURL(directory + "/"));
  } finally { WebAssembly.instantiate = original; }
  const input = { x: Float64Array.from({ length: 1001 }, (_, i) => i), y: Array(1001).fill(null) };
  counts.clear(); const source = aq.table(input);
  assert.equal(counts.get("cell-set") ?? 0, 0); assert.equal(counts.get("numeric-column"), 2);
  counts.clear(); const result = source.objects();
  assert.equal(result.length, 1001); assert.equal(counts.get("get") ?? 0, 0); assert.equal(counts.get("column-export"), 2);
  assert.equal(api["transfer-live"](), 0); const capacity = api.memory.buffer.byteLength;
  for (let i = 0; i < 20; i++) assert.equal(source.array("x")[1000], 1000);
  assert.equal(api.memory.buffer.byteLength, capacity); assert.equal(api["transfer-live"](), 0);
  assert.ok(capacity <= 10 * 65536);
  assert.throws(() => aq.table({ x: [1, "x"] })); assert.equal(api["transfer-live"](), 0);
  const invalid = Array(65537).fill(1); invalid[65536] = "wrong type";
  assert.throws(() => aq.table({ x: invalid })); assert.equal(api["transfer-live"](), 0);
  assert.equal(source.array("x")[1000], 1000);
});

test("numeric ingestion stays unboxed and lazy views preserve Cell identity", async () => {
  const api = (await WebAssembly.instantiate(await readFile(join(directory, "core.wasm")))).instance.exports;
  assert.equal(typeof api["numeric-new"], "function");
  const empty = api.text(api["bytes-new"](0)), builder = api["numeric-new"](3);
  assert.throws(() => api["numeric-new"](-1), WebAssembly.RuntimeError);
  assert.throws(() => api["numeric-import"](builder, 0, 1), WebAssembly.RuntimeError);
  for (const [kind, number, start, n] of [[99, 0, 0, 1], [2, NaN, 0, 1], [2, 2, 0, 1], [3, 0, 3, 1], [3, 0, -1, 1], [3, 0, 0, 4]]) {
    api["transfer-open"](16); new Uint8Array(api.memory.buffer)[16] = kind;
    new Float64Array(api.memory.buffer, 24, 1)[0] = number;
    try { assert.throws(() => api["numeric-import"](builder, start, n), WebAssembly.RuntimeError); }
    finally { api["transfer-close"](); }
    assert.equal(api["transfer-live"](), 0);
  }
  api["transfer-open"](32);
  new Uint8Array(api.memory.buffer, 16, 3).set([3, 1, 2]);
  new Float64Array(api.memory.buffer, 24, 3).set([-0, 42, 1]);
  api["numeric-import"](builder, 0, 3); api["transfer-close"]();
  const c = api["numeric-column"](empty, builder, empty);
  assert.equal(api["column-boxed-count"](c), 0);
  const cols = api["columns-new"](1); api["column-set"](cols, 0, c); const t = api["table-new"](cols, 3);
  assert.equal(api["column-boxed-count"](c), 0, "schema validation must not materialize a Cell list");
  const first = api.get(t, 0, 0); assert.strictEqual(api.get(t, 0, 0), first);
  assert.ok(Object.is(api["cell-number"](first), -0)); assert.equal(api["column-boxed-count"](c), 1);
  const view = api["column-values"](c); assert.strictEqual(api["column-values"](c), view);
  assert.strictEqual(api.get(t, 0, 0), first); assert.equal(api["column-boxed-count"](c), 3);
  assert.equal(api["cell-number"](api.get(t, 0, 1)), 42, "unused null payloads remain observable through the raw Cell accessor");
  api["transfer-open"](32); new Uint8Array(api.memory.buffer, 16, 3).set([3, 3, 3]);
  new Float64Array(api.memory.buffer, 24, 3).fill(99);
  api["numeric-import"](builder, 0, 3); api["transfer-close"]();
  assert.strictEqual(api.get(t, 0, 0), first, "published storage snapshots its mutable builder");
});
