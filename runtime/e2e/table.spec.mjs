import { test, expect } from "@playwright/test";

test("resident table facade performs relational pipelines and joins in Chromium", async ({ page }) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const base = new URL("/table/", location.href), { entry } = await (await fetch(new URL("entry.json", base))).json();
    const { instantiate, op, desc } = await import(new URL(entry, base));
    const aq = await instantiate(base);
    const input = { country: ["JP", "US", "JP", "FR"], amount: [20, 100, 22, 3], units: [2, 1, 3, 4] };
    const source = aq.table(input);
    const rows = source.filter(d => d.amount >= 10).derive({ revenue: d => d.amount * d.units }).groupby("country")
      .rollup({ count: op.count(), revenue: op.sum("revenue") }).orderby(desc("revenue")).objects();
    const a = op;
    const aggregateAlias = source.rollup({ total: d => a.sum(d.amount * d.units) }).objects();
    const joined = aq.table({ k: [1, 2, 2] }).join_left(aq.table({ k: [2, 2], label: ["B", "C"] }), "k").objects();
    const sorted = aq.table({ name: ["\uffff", "🦀", "\ufeff", ""] }).orderby("name").array("name");
    input.amount[0] = 999;
    let rejected = false; try { source.filter(d => alert(d.country)); } catch { rejected = true; }
    const modes = [];
    for (const filter of ["simd", "scalar", "packed-scalar"]) {
      const engine = await instantiate(base, { filter });
      const table = engine.table({ x: [undefined, null, NaN, -0, 0, 1, -1, Infinity, -Infinity] });
      modes.push([table.filter(d => d.x == 0).numRows(), table.filter(d => d.x === 0).numRows(),
        table.orderby("x").slice(1).filter(d => !(d.x < 0) && d.x !== 0).numRows()]);
    }
    const large = aq.table({ x: Float64Array.from({ length: 65537 }, (_, i) => i) });
    const special = aq.table({ ["__proto__"]: [7, null], b: [true, false] }).objects();
    const raw = (await WebAssembly.instantiate(await (await fetch(new URL("core.wasm", base))).arrayBuffer())).instance.exports;
    const empty = raw.text(raw["bytes-new"](0)); let transferTrapped = false;
    const builder = raw["numeric-new"](1);
    raw["transfer-open"](16); new Uint8Array(raw.memory.buffer)[16] = 3;
    new Float64Array(raw.memory.buffer, 24, 1)[0] = -0;
    raw["numeric-import"](builder, 0, 1); raw["transfer-close"]();
    const column = raw["numeric-column"](empty, builder, empty), columns = raw["columns-new"](1);
    raw["column-set"](columns, 0, column); const numericTable = raw["table-new"](columns, 1);
    const unboxed = raw["column-boxed-count"](column), cell = raw.get(numericTable, 0, 0);
    const sameCell = cell === raw.get(numericTable, 0, 0), view = raw["column-values"](column);
    const sameView = view === raw["column-values"](column) && cell === raw.get(numericTable, 0, 0);
    raw["transfer-open"](589824); new Uint8Array(raw.memory.buffer)[16] = 99;
    try { raw["column-import"](empty, 1, empty); } catch { transferTrapped = true; } finally { raw["transfer-close"](); }
    return { rows, aggregateAlias, joined, sorted, snapshot: source.get("amount", 0), rejected, modes,
      transfer: { last: large.array("x")[65536], count: large.numRows(), transferTrapped,
        unboxed, sameCell, sameView, negativeZero: Object.is(raw["cell-number"](cell), -0),
        live: raw["transfer-live"](), bytes: raw.memory.buffer.byteLength, proto: special.map(row => row["__proto__"]) } };
  });
  expect(result).toEqual({ rows: [{ country: "JP", count: 2, revenue: 106 }, { country: "US", count: 1, revenue: 100 }],
    aggregateAlias: [{ total: 218 }],
    joined: [{ k: 2, label: "B" }, { k: 2, label: "C" }, { k: 2, label: "B" }, { k: 2, label: "C" }, { k: 1, label: undefined }],
    sorted: ["", "🦀", "\ufeff", "\uffff"], snapshot: 20, rejected: true,
    modes: [[2, 2, 4], [2, 2, 4], [2, 2, 4]],
    transfer: { last: 65536, count: 65537, transferTrapped: true, unboxed: 0, sameCell: true, sameView: true,
      negativeZero: true, live: 0, bytes: 655360, proto: [7, null] } });
});

for (const backend of ["core", "component"]) test(`${backend}: portable table snapshots preserve scalar values and clean ABI allocations`, async ({ page }) => {
  await page.goto(`/${backend}/`);
  const result = await page.evaluate(async () => {
    const { instantiate } = await import(new URL("table/index.js", location.href)); let arena;
    const { api } = await instantiate({ instantiateCore: async (m, imports) => {
      const value = await WebAssembly.instantiate(m, imports); if (value.exports.live) arena = value.exports; return value;
    } });
    const values = [{ kind: 3, number: 42, text: "" }, { kind: 4, number: 0, text: "日本🦀" }, { kind: 1, number: 0, text: "" }];
    const output = api.run({ length: 3, columns: [{ name: "value", values }] }, []);
    const value = { kind: 3, number: 0, text: "" };
    const filter = { op: 1, names: [], indices: [], other: { length: 0, columns: [] }, leftKeys: [], rightKeys: [], layout: [], mode: 0,
      programs: [{ nodes: [{ op: 1, value, a: 0, b: -1, c: -1 }, { op: 0, value, a: -1, b: -1, c: -1 },
        { op: 25, value, a: 0, b: 1, c: 1 }], root: 2 }] };
    const numeric = { length: 5, columns: [{ name: "x", values: [
      { kind: 0, number: 0, text: "" }, { kind: 1, number: 0, text: "" },
      { kind: 3, number: NaN, text: "" }, { kind: 3, number: -1, text: "" }, value,
    ] }] };
    const modes = [api.run, api.runScalar, api.runPackedScalar].map(run => run(numeric, [filter]).length);
    let trapped = false; try { api.run({ length: 2, columns: [{ name: "value", values }] }, []); } catch { trapped = true; }
    return { output, modes, trapped, live: arena.live() };
  });
  expect(result).toEqual({ output: { length: 3, columns: [{ name: "value", values: [
    { kind: 3, number: 42, text: "" }, { kind: 4, number: 0, text: "日本🦀" }, { kind: 1, number: 0, text: "" },
  ] }] }, modes: [2, 2, 2], trapped: true, live: 0 });
});
