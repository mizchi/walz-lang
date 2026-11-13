import test from "node:test";
import assert from "node:assert/strict";
import { dataset, encode, validate, histogram, postings, countsRaw, countsEncoded, countsHistogram,
  countsValidated, queryRaw, queryEncoded, queryPostings } from "./dictionary-bench/workloads.mjs";

test("dictionary benchmark controls preserve complete results and distinguish precomputed indexes", () => {
  for (const [n, k, unicode] of [[1, 1, false], [129, 4, false], [1025, 64, true]]) {
    const data = dataset(n, k, unicode), encoded = encode(data.values), ids = new Map(encoded.dictionary.map((key, i) => [key, i]));
    assert.deepEqual(encoded.dictionary, data.vocabulary.slice(0, k));
    assert.deepEqual(encoded.codes, data.ids);
    assert.deepEqual(validate(encoded), encoded);
    const counts = Int32Array.from(data.queries, key => data.values.filter(value => value === key).length);
    for (const actual of [countsRaw(data.values, data.queries), countsEncoded(encoded, data.queries),
      countsEncoded(encoded, data.queries, ids), countsHistogram(histogram(data.values), data.queries),
      countsValidated(encoded, data.queries)]) assert.deepEqual(actual, counts);
    const index = postings(encoded);
    for (const key of [data.vocabulary[0], data.vocabulary.at(-1)]) {
      const rows = data.values.flatMap((value, i) => value === key && data.amounts[i] >= data.lo && data.amounts[i] < data.hi ? [i] : []);
      const expected = { count: rows.length, sum: rows.reduce((sum, i) => sum + data.amounts[i], 0), indices: Int32Array.from(rows) };
      for (const actual of [queryRaw(data.values, data.amounts, key, data.lo, data.hi),
        queryEncoded(encoded, data.amounts, key, data.lo, data.hi), queryEncoded(encoded, data.amounts, key, data.lo, data.hi, true),
        queryPostings(index, data.amounts, key, data.lo, data.hi)]) assert.deepEqual(actual, expected);
    }
  }
});
test("dictionary benchmark references retain zero IDs, UTF-8 keys and strict validation", () => {
  const values = ["", "\u0000", "é", "e\u0301", "", "日本🦀"], encoded = encode(values);
  assert.deepEqual(encoded.codes, Int32Array.of(0, 1, 2, 3, 0, 4));
  assert.deepEqual(validate({ dictionary: ["unused", ...encoded.dictionary], codes: Int32Array.of(1, 5) }).codes, Int32Array.of(1, 5));
  for (const value of [{ dictionary: ["a", "a"], codes: new Int32Array() },
    { dictionary: [], codes: Int32Array.of(0) }, { dictionary: ["a"], codes: Int32Array.of(-1) },
    { dictionary: ["a"], codes: Int32Array.of(1) }]) {
    assert.throws(() => validate(value), RangeError);
    assert.throws(() => countsValidated(value, []), RangeError);
  }
  assert.deepEqual(countsHistogram(histogram(values), ["", "missing"]), Int32Array.of(2, 0));
});
