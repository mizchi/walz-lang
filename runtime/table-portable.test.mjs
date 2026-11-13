import test from "node:test";
import assert from "node:assert/strict";
import * as aq from "arquero";
import { op } from "./table/index.mjs";
import { snapshot, objects, program, command } from "./testing/table-wire.mjs";
import { instrument } from "./testing/memory.mjs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

for (const backend of ["core", "component"]) test(`${backend}: a table pipeline runs through the value ABI and releases all buffers`, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-table-portable-"));
  try {
    const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
    await build("examples/table/portable.walz", directory); await instrument(directory, backend);
    const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
    async function create() {
      let arena;
      const value = await instantiate({ instantiateCore: async (m, imports) => {
        const result = await WebAssembly.instantiate(m, imports); if (result.exports.live) arena = result.exports; return result;
      } });
      return { api: value.api, clean() { assert.equal(arena.live(), 0); assert.ok(Object.values(value.__memory()).every(n => n === 0)); } };
    }
    const { api, clean } = await create();
    const names = ["country", "amount", "units"];
    const steps = [
      command(1, { programs: [program(d => d.amount >= 10, names)] }),
      command(2, { names: ["revenue"], programs: [program(d => d.amount * d.units, names)] }),
      command(3, { names: ["country"], programs: [program("country", [...names, "revenue"], { column: true })] }),
      command(4, { names: ["count", "revenue"], programs: [program(op.count(), [...names, "revenue"], { aggregate: true }), program(op.sum("revenue"), [...names, "revenue"], { aggregate: true })] }),
      command(5, { programs: [program("revenue", ["country", "count", "revenue"], { column: true })], indices: [1] }),
    ];
    const data = { country: ["JP", "US", "JP", "FR"], amount: [20, 100, 22, 3], units: [2, 1, 3, 4] };
    const expected = aq.table(data).filter(d => d.amount >= 10).derive({ revenue: d => d.amount * d.units }).groupby("country")
      .rollup({ count: aq.op.count(), revenue: aq.op.sum("revenue") }).orderby(aq.desc("revenue")).objects();
    assert.deepEqual(objects(api.run(snapshot(data), steps)), expected); clean();
    for (const run of [api.run, api.runScalar, api.runPackedScalar]) {
      assert.deepEqual(objects(run(snapshot(data), steps)), expected); clean();
      // The wire ABI can supply mixed tags even though the JS facade uses
      // homogeneous input columns. SIMD must never infer tags from row zero.
      const mixed = { x: [undefined, null, false, true, NaN, -0, 0, -1, Infinity] };
      for (const predicate of [d => d.x === 0, d => d.x == 0, d => d.x !== 0,
        d => d.x != 0, d => !(d.x < 0), d => d.x >= 0 && d.x < 2, d => 0 < d.x]) {
        const result = run(snapshot(mixed), [command(1, { programs: [program(predicate, ["x"])] })]);
        assert.deepEqual(objects(result), aq.table(mixed).filter(predicate).objects()); clean();
      }
      const trapped = await create();
      const missing = program(d => false && d.x > 0, ["x"]);
      missing.nodes.find(node => node.op === 1).a = 100;
      assert.throws(() => trapped.api[run === api.run ? "run" : run === api.runScalar ? "runScalar" : "runPackedScalar"](
        snapshot({ x: [] }), [command(1, { programs: [missing] })]), WebAssembly.RuntimeError);
      trapped.clean();
    }
    for (const value of [{ x: [] }, { x: [undefined, null, NaN, -0, Infinity, -Infinity] }, { x: ["🦀", "\uffff", "", "\ufeff", "\u0000"] }]) {
      assert.deepEqual(objects(api.run(snapshot(value), [])), aq.table(value).objects()); clean();
    }
    const left = { k: [1, 2, 2], x: [10, 20, 30] }, right = { k: [2, 2, 3], y: [40, 50, 60] };
    const joinStep = command(8, { other: snapshot(right), leftKeys: [0], rightKeys: [0], mode: 3,
      layout: [{ name: "k", side: 2, left: 0, right: 0 }, { name: "x", side: 0, left: 1, right: 0 }, { name: "y", side: 1, left: 0, right: 1 }] });
    assert.deepEqual(objects(api.run(snapshot(left), [joinStep])), aq.table(left).join_full(aq.table(right), "k").objects()); clean();
    const invalid = program(d => d.amount + 1, names); invalid.nodes[invalid.root].a = invalid.root;
    for (const [input, commands] of [
      [snapshot(data), [command(999)]], [snapshot(data), [command(1, { programs: [invalid] })]],
      [{ columns: snapshot({ x: [1] }).columns, length: 2 }, []],
      [{ columns: [{ name: "x", values: [{ kind: 2, number: NaN, text: "" }] }], length: 1 }, []],
    ]) { const fresh = await create(); assert.throws(() => fresh.api.run(input, commands), WebAssembly.RuntimeError); fresh.clean(); }
  } finally { await rm(directory, { recursive: true, force: true }); }
});
