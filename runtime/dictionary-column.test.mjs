import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { collect, instrument } from "./testing/memory.mjs";

const exec = promisify(callback), root = new URL("..", import.meta.url);
function encode(values) {
  const dictionary = [], codes = [], ids = new Map();
  for (const original of values) {
    const value = original.toWellFormed(); // UTF-8 replaces unpaired surrogates.
    if (!ids.has(value)) { ids.set(value, dictionary.length); dictionary.push(value); }
    codes.push(ids.get(value));
  }
  return { dictionary, codes };
}
const normalize = value => ({ dictionary: value.dictionary, codes: Array.from(value.codes) });
const selected = (values, keys) => values.flatMap((value, i) => keys.includes(value) ? [i] : []);

for (const backend of ["core", "component"]) test(`${backend}: dictionary columns roundtrip exact values, validate codes, and compose SIMD selections`, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-dictionary-"));
  try {
    const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
    await build("examples/dictionary-column/portable.walz", directory); await instrument(directory, backend);
    const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
    async function create() {
      let arena;
      const value = await instantiate({ instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } });
      return { api: value.api, clean() { assert.equal(arena.live(), 0); assert.ok(Object.values(value.__memory()).every(n => n === 0)); } };
    }
    const { api, clean } = await create();
    const vocabulary = ["", "JP", "US", "日本🦀", "\u0000", "é", "e\u0301", "\ufeffBOM"];
    const cases = [[], ["JP"], ["JP", "US", "JP"], ["\ud800", "\ufffd", "\udc00"]];
    for (const n of [3, 4, 5, 31, 32, 33, 127, 128, 129, 257, 1025])
      cases.push(Array.from({ length: n }, (_, i) => vocabulary[(i * 5) % vocabulary.length]));
    cases.push(Array.from({ length: 300 }, (_, i) => `key-${i}`).concat(["key-0", "key-299"]));
    for (const input of cases) {
      const values = input.map(v => v.toWellFormed()), expected = encode(input), snapshot = api.encode(input);
      assert.deepEqual(normalize(snapshot), expected);
      assert.deepEqual(api.decode(snapshot), values);
      for (const queries of [[], ["JP", "missing", "JP"], ["", "\u0000", "日本🦀"], ["é", "e\u0301", "\ufeffBOM"]]) {
        const counts = queries.map(key => values.filter(value => value === key).length);
        assert.deepEqual(Array.from(api.countMany(input, queries)), counts);
        assert.deepEqual(Array.from(api.countEncoded(snapshot, queries)), counts);
        assert.deepEqual(Array.from(api.select(input, queries)), selected(values, queries));
      }
      if (values.length) assert.equal(api.get(snapshot, values.length - 1), values.at(-1));
      assert.deepEqual(Array.from(api.equal(snapshot, "JP")), selected(values, ["JP"]));
      if (snapshot.codes.length) snapshot.codes[0] = 123;
      if (snapshot.dictionary.length) snapshot.dictionary[0] = "changed";
      assert.deepEqual(normalize(api.encode(input)), expected); clean();
    }
    const supplied = { dictionary: ["unused", "JP", "US"], codes: [2, 1, 2] };
    assert.deepEqual(api.decode(supplied), ["US", "JP", "US"]);
    assert.deepEqual(Array.from(api.countEncoded(supplied, ["unused", "JP", "US"])), [0, 1, 2]);
    const wide = { dictionary: Array.from({ length: 65537 }, (_, i) => `key-${i}`), codes: [65536, 256, 0] };
    assert.deepEqual(api.decode(wide), ["key-65536", "key-256", "key-0"], "i32 codes are not silently narrowed");
    clean();
    const answer = api.query(["JP", "US", "JP", "FR"], [20, 100, 22, 3], ["JP", "JP", "missing"], 10, 30);
    assert.equal(answer.count, 2); assert.equal(answer.sum, 42n); assert.deepEqual(Array.from(answer.indices), [0, 2]); clean();
    // Compare the fused kernel with an independent scalar/BigInt oracle and
    // the existing bitmap composition, including dense results and growth.
    for (const n of [0, 1, 2, 3, 4, 5, 31, 32, 33, 127, 128, 129, 1025, 4099]) {
      for (const dense of [false, true]) {
        const labels = Array.from({ length: n }, (_, i) => dense ? "JP" : vocabulary[i % vocabulary.length]);
        const amounts = Int32Array.from({ length: n }, (_, i) => [0, -1, 1, -2147483648, 2147483647, 2147483646][i % 6]);
        for (const keys of [[], ["missing"], ["JP", "JP"], ["JP", "US", "", "日本🦀", "missing", "JP"]]) {
          for (const [lo, hi] of [[-1, 2], [-2147483648, 2147483647], [2147483646, 2147483647], [0, 0], [1, -1]]) {
            const indices = labels.flatMap((value, i) => keys.includes(value) && amounts[i] >= lo && amounts[i] < hi ? [i] : []);
            const expected = { count: indices.length, sum: indices.reduce((sum, i) => sum + BigInt(amounts[i]), 0n), indices };
            for (const fn of [api.query, api.queryComposed]) {
              const actual = fn(labels, amounts, keys, lo, hi);
              assert.deepEqual({ ...actual, indices: Array.from(actual.indices) }, expected, `${backend}/${n}/${dense}/${keys}/${lo}/${hi}`);
              clean();
            }
          }
        }
      }
    }
    for (const operation of [
      api => api.decode({ dictionary: ["a", "a"], codes: [] }),
      api => api.decode({ dictionary: [], codes: [0] }),
      api => api.decode({ dictionary: ["a"], codes: [-1] }),
      api => api.decode({ dictionary: ["a"], codes: [1] }),
      api => api.decode({ dictionary: ["a"], codes: [2147483647] }),
      api => api.get({ dictionary: ["a"], codes: [0] }, -1),
      api => api.get({ dictionary: ["a"], codes: [0] }, 1),
      api => api.query(["JP"], [], ["missing"], 0, 1),
      api => api.query([], [0], [], 0, 0),
      api => api.query(["JP"], [], [], 0, 0),
      api => api.query(["JP"], [0, 0], ["JP"], 1, -1),
    ]) { const fresh = await create(); assert.throws(() => operation(fresh.api), WebAssembly.RuntimeError); fresh.clean(); }
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("dictionary columns release construction inputs and keep dictionary, codes and selections independently", { skip: typeof global.gc !== "function" }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-dictionary-gc-"));
  try {
    const file = join(directory, "resident.wasm");
    await exec("cargo", ["run", "--locked", "--quiet", "--", "build", "examples/dictionary-column/resident.walz", file], { cwd: root });
    const api = (await WebAssembly.instantiate(await readFile(file))).instance.exports;
    const input = () => { const value = api.input(129); for (let i = 0; i < 129; i++) api.set(value, i, i % 3 + 1001); return value; };
    async function released(refs) {
      for (let pass = 0; pass < 8; pass++) { await collect(); if (refs.every(ref => ref.deref() === undefined)) return; }
      assert.fail("unneeded source or column remains reachable");
    }
    const snapshot = (() => { const source = input(); return { column: api.pack(source), ref: new WeakRef(source) }; })();
    await released([snapshot.ref]); assert.equal(api.count(snapshot.column, 1001), 43);
    const held = new WeakRef(snapshot.column); await collect(); assert.ok(held.deref()); assert.equal(api.len(snapshot.column), 129);
    for (const mode of ["codes", "dictionary", "selection"]) {
      const history = (() => {
        const source = input(), column = api.pack(source), keys = api.dictionary(column), codes = api.codes(column);
        const selection = api.equal(column, 1001);
        const keep = mode === "codes" ? codes : mode === "dictionary" ? keys : selection;
        return { keep, refs: [new WeakRef(source), new WeakRef(column),
          ...(mode !== "codes" ? [new WeakRef(codes)] : []),
          ...(mode !== "dictionary" ? [new WeakRef(keys)] : [])] };
      })();
      await released(history.refs);
      if (mode === "codes") assert.equal(api["code-get"](history.keep, 128), 2);
      else if (mode === "dictionary") assert.equal(api["key-length"](history.keep, 0), 4);
      else assert.equal(api["bitmap-count"](history.keep), 43);
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("fused query results release source columns and keep independent row snapshots", { skip: typeof global.gc !== "function" }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-dictionary-query-gc-"));
  try {
    const file = join(directory, "resident.wasm");
    await exec("cargo", ["run", "--locked", "--quiet", "--", "build", "examples/dictionary-column/bench-resident.walz", file], { cwd: root });
    const api = (await WebAssembly.instantiate(await readFile(file))).instance.exports;
    for (const keepRows of [false, true]) {
      const history = (() => {
        const source = api["strings-new"](129), values = api["numbers-new"](129), keys = api["strings-new"](1);
        api["label-set"](keys, 0, 0, 0);
        for (let i = 0; i < 129; i++) { api["label-copy"](source, i, keys, 0); api["number-set"](values, i, 42); }
        const column = api.encode(api["strings-freeze"](source));
        const amounts = api.amounts(api["numbers-freeze"](values));
        const answer = api["query-fused"](column, amounts, api["strings-freeze"](keys), 0, 100);
        return { keep: keepRows ? api["answer-indices"](answer) : answer,
          refs: [source, values, keys, column, amounts, ...(keepRows ? [answer] : [])].map(value => new WeakRef(value)) };
      })();
      for (let pass = 0; pass < 8; pass++) {
        await collect(); if (history.refs.every(ref => ref.deref() === undefined)) break;
      }
      assert.ok(history.refs.every(ref => ref.deref() === undefined));
      const positive = new WeakRef(history.keep); await collect(); assert.ok(positive.deref());
      const rows = keepRows ? history.keep : api["answer-indices"](history.keep);
      assert.equal(api["counts-len"](rows), 129); assert.equal(api["counts-get"](rows, 128), 128);
      if (!keepRows) { assert.equal(api["answer-count"](history.keep), 129); assert.equal(api["answer-sum"](history.keep), 5418n); }
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});
