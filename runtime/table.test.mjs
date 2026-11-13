import test, { before, after } from "node:test";
import assert from "node:assert/strict";
import * as reference from "arquero";
import { instantiate, op, desc } from "./table/index.mjs";
import { build } from "./table/build.mjs";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { collect } from "./testing/memory.mjs";

let directory, temporary;
before(async () => {
  temporary = !process.env.WALZ_TABLE_KERNEL;
  directory = temporary ? await mkdtemp(join(tmpdir(), "walz-table-")) : resolve(process.env.WALZ_TABLE_KERNEL);
  if (temporary) await build(directory);
});
after(async () => { if (temporary) await rm(directory, { recursive: true, force: true }); });

test("numeric filter execution modes preserve tags, tails, sparse order and short circuiting", async () => {
  const base = pathToFileURL(directory + "/");
  await assert.rejects(instantiate(base, { filter: "invalid" }), /filter/);
  const predicates = [
    d => d.x < 0, d => d.x <= 0, d => d.x > 0, d => d.x >= 0,
    d => d.x === 0, d => d.x !== 0, d => d.x == 0, d => d.x != 0,
    d => 0 < d.x, d => 0 <= d.x, d => 0 > d.x, d => 0 >= d.x,
    d => !(d.x < 0), d => d.x >= -1 && d.x < 3,
    d => d.x === 0 || d.x > 2, d => false && (d.text - 1),
    d => d.x === NaN, d => d.x !== NaN, d => d.x == NaN, d => d.x != NaN,
    d => d.x >= Infinity, d => d.x <= -Infinity, d => d.x === -0, d => d.x >= +0,
    d => d.x < 0 || d.x >= 2, d => d.x >= 0 && 2 > d.x,
    d => d.x >= 0 && d.id < 5, d => d.x < 0 || d.id >= 5,
    d => d.x < Infinity && d.x >= -Infinity,
  ];
  const numbers = [undefined, null, NaN, 0, -0, Infinity, -Infinity, 1, -1, 0.5, Number.MIN_VALUE, 2 ** 53];
  for (const mode of ["simd", "scalar", "packed-scalar"]) {
    const aq = await instantiate(base, { filter: mode });
    for (const n of [0, 1, 2, 3, 4, 5, 31, 32, 33, 129]) {
      for (const values of [numbers, [true, false, null, undefined]]) {
        const data = { x: Array.from({ length: n }, (_, i) => values[i % values.length]),
          id: Array.from({ length: n }, (_, i) => i), text: Array(n).fill("text") };
        const input = aq.table(data), expected = reference.table(data);
        for (const predicate of predicates) {
          assert.deepEqual(input.filter(predicate).objects(), expected.filter(predicate).objects(), `${mode}/${n}/${predicate}`);
          assert.deepEqual(input.orderby(desc("id")).slice(1).filter(d => d.id >= 2).filter(predicate).objects(),
            expected.orderby(reference.desc("id")).slice(1).filter(d => d.id >= 2).filter(predicate).objects());
        }
      }
    }
    assert.throws(() => aq.table({ x: [1], text: ["x"] }).filter(d => d.x > 0 && d.text - 1), WebAssembly.RuntimeError);
    assert.throws(() => aq.table({ x: [] }).filter(d => false && d.missing > 0));
    const t = aq.table({ x: [1e16, 1, -1e16] });
    assert.equal(t.filter(d => d.x !== 0).rollup({ sum: op.sum("x") }).get("sum", 0), 0);
  }
});

test("native table constructors reject malformed values, schemas and expression nodes", async () => {
  const api = (await WebAssembly.instantiate(await readFile(join(directory, "core.wasm")))).instance.exports;
  const text = api.text(api["bytes-new"](0)), value = api["cell-new"](3, 42, text), expr = api.literal(value);
  const empty = api["table-new"](api["columns-new"](0), 0);
  assert.throws(() => api.filter(empty, api.field(0)), WebAssembly.RuntimeError);
  assert.throws(() => api.filter(empty, api.node(100, expr, expr, expr)), WebAssembly.RuntimeError);
  assert.throws(() => api.node(999, expr, expr, expr), WebAssembly.RuntimeError);
  assert.throws(() => api["cell-new"](2, NaN, text), WebAssembly.RuntimeError);
  assert.throws(() => api["cell-new"](99, 0, text), WebAssembly.RuntimeError);
  const cells = api["cells-new"](1); api["cell-set"](cells, 0, value);
  const col = api["column-new"](text, cells), columns = api["columns-new"](1); api["column-set"](columns, 0, col);
  const table = api["table-new"](columns, 1);
  api["cell-set"](cells, 0, api["cell-new"](3, 99, text));
  assert.equal(api["cell-number"](api.get(table, 0, 0)), 42, "the published column is an independent snapshot");
  const wrong = api["columns-new"](1); api["column-set"](wrong, 0, api["column-new"](text, api["cells-new"](0)));
  assert.throws(() => api.select(table, wrong), WebAssembly.RuntimeError);
  assert.throws(() => api.groupby(table, wrong), WebAssembly.RuntimeError);
  const duplicates = api["columns-new"](2); api["column-set"](duplicates, 0, col); api["column-set"](duplicates, 1, col);
  assert.throws(() => api.select(table, duplicates), WebAssembly.RuntimeError);
  assert.throws(() => api["table-new"](columns, -1), WebAssembly.RuntimeError);
});


test("Walz table verbs agree with Arquero on immutable column pipelines", async () => {
    const aq = await instantiate(pathToFileURL(directory + "/"));
    const data = { country: ["JP", "US", "JP", "FR", "US"], amount: [20, 100, 22, null, 30], units: [2, 1, 3, 4, 2] };
    const source = aq.table(data), original = reference.table(data);
    const pipelines = [
      (t, op, desc) => t.select("amount", "country"),
      (t, op, desc) => t.filter(d => d.country === "JP" && d.amount >= 20),
      (t, op, desc) => t.derive({ total: d => d.amount * d.units, amount: d => d.amount + 1 }),
      (t, op, desc) => t.groupby("country").rollup({ count: op.count(), total: op.sum("amount"), avg: op.mean("amount"), min: op.min("amount"), max: op.max("amount") }),
      (t, op, desc) => t.filter(d => d.amount != null).groupby("country").rollup({ total: op.sum("amount") }).orderby(desc("total")),
      (t, op, desc) => t.groupby("country").select("amount").rollup({ total: op.sum("amount") }),
      (t, op, desc) => t.orderby("country", desc("units")).slice(1, 4),
    ];
    for (const pipeline of pipelines) {
      const actual = pipeline(source, op, desc);
      const expected = pipeline(original, reference.op, reference.desc);
      assert.deepEqual(actual.objects(), expected.objects());
      assert.deepEqual(actual.columnNames(), expected.columnNames());
      assert.equal(actual.numRows(), expected.numRows());
    }
    assert.deepEqual(source.objects(), original.objects(), "verbs do not mutate their inputs");
    data.amount[0] = 999; assert.equal(source.get("amount", 0), 20, "ingestion snapshots mutable JS input");
    assert.deepEqual(source.select().objects(), Array.from({ length: 5 }, () => ({})));
    for (const n of [0, 1, 3, 4, 5, 31, 32, 33, 129, 1025]) {
      const values = { group: Array.from({ length: n }, (_, i) => `g${i % 7}`), x: Array.from({ length: n }, (_, i) => i % 11 ? i - 100 : NaN) };
      const a = aq.table(values).filter(d => d.x >= -50).derive({ twice: d => d.x * 2 }).groupby("group").rollup({ n: op.count(), sum: op.sum("twice") }).orderby("group");
      const b = reference.table(values).filter(d => d.x >= -50).derive({ twice: d => d.x * 2 }).groupby("group").rollup({ n: reference.op.count(), sum: reference.op.sum("twice") }).orderby("group");
      assert.deepEqual(a.objects(), b.objects());
    }
});

test("Walz tables preserve null, undefined, NaN, stable order, Unicode and join multiplicity", async () => {
    const aq = await instantiate(pathToFileURL(directory + "/"));
    const data = { k: ["🦀", "\uffff", "", "é", "e\u0301", "\u0000", "\ufeff"], v: [NaN, undefined, null, 2, -1, 0, -0] };
    for (const columns of [data, { k: [0, -0, NaN, null, undefined, 0], v: [1, 2, 3, 4, 5, 6] }]) {
      assert.deepEqual(aq.table(columns).orderby("k").objects(), reference.table(columns).orderby("k").objects());
      assert.deepEqual(aq.table(columns).groupby("k").count().objects(), reference.table(columns).groupby("k").count().objects());
      assert.deepEqual(aq.table(columns).orderby("v").groupby("k").count().objects(), reference.table(columns).orderby("v").groupby("k").count().objects());
    }
    for (const v of [[], [null, undefined, NaN], [1, 2, NaN, null, undefined], [-0, 0, Infinity, -Infinity]]) {
      const specs = x => ({ count: x.count(), valid: x.valid("v"), invalid: x.invalid("v"), sum: x.sum("v"), mean: x.mean("v"), min: x.min("v"), max: x.max("v"), distinct: x.distinct("v") });
      assert.deepEqual(aq.table({ v }).rollup(specs(op)).objects(), reference.table({ v }).rollup(specs(reference.op)).objects());
    }
    const left = { key: [1, 2, 2, null, 9], value: ["a", "b", "c", "null", "z"] };
    const right = { key: [2, 2, 3, NaN], value: ["B", "C", "D", "nan"] };
    for (const [l, r] of [[left, right], [right, left]]) for (const method of ["join", "join_left", "join_right", "join_full", "semijoin", "antijoin"]) {
      assert.deepEqual(aq.table(l)[method](aq.table(r), "key").objects(), reference.table(l)[method](reference.table(r), "key").objects(), method);
    }
    for (const action of [
      () => aq.table({ a: [1], b: [] }), () => aq.table({ a: [{}] }),
      () => aq.table({ a: [1] }).filter(d => missing(d.a)),
      () => aq.table({ a: [1] }).derive({ b: d => d.missing + 1 }),
      () => aq.table({ a: ["\ud800"] }),
      () => aq.table({ a: [] }).rollup({ a: d => d.a }),
      () => op.sum("a", "b"),
    ]) assert.throws(action);
    const ordered = { k: ["b", "a", "b", "c"], x: [2, 1, 0, 3] };
    assert.deepEqual(aq.table(ordered).orderby("x").slice(0, 3).groupby("k").count().objects(),
      reference.table(ordered).orderby("x").slice(0, 3).groupby("k").count().objects());
});

test("retained Walz tables release JS sources and discarded table wrappers", { skip: typeof global.gc !== "function" }, async () => {
    const aq = await instantiate(pathToFileURL(directory + "/"));
    const history = (() => {
      const input = { x: Array.from({ length: 129 }, (_, i) => i) }, source = aq.table(input);
      return { keep: source.filter(d => d.x >= 64), refs: [new WeakRef(input), new WeakRef(input.x), new WeakRef(source)] };
    })();
    for (let i = 0; i < 8; i++) { await collect(); if (history.refs.every(ref => ref.deref() === undefined)) break; }
    assert.ok(history.refs.every(ref => ref.deref() === undefined)); assert.equal(history.keep.numRows(), 65);
    assert.equal(history.keep.rollup({ sum: op.sum("x") }).get("sum", 0), 6240);
});
