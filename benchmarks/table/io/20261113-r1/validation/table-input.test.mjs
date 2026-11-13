import test from "node:test";
import assert from "node:assert/strict";
import { readChunk } from "./table/input.mjs";

test("input chunks snapshot tags and unboxed payloads while reading each element once", () => {
  const values = [undefined, null, NaN, -0, Infinity]; let reads = 0;
  Object.defineProperty(values, 3, { get() { reads++; return -0; }, configurable: true });
  const chunk = readChunk(values, 0, values.length, undefined, "x");
  assert.ok(chunk.numbers instanceof Float64Array); assert.ok(chunk.kinds instanceof Uint8Array);
  assert.equal(chunk.strings, undefined); assert.deepEqual([...chunk.kinds], [0, 1, 3, 3, 3]);
  assert.ok(Number.isNaN(chunk.numbers[2])); assert.ok(Object.is(chunk.numbers[3], -0)); assert.equal(reads, 1);
  values[4] = 42; assert.equal(chunk.numbers[4], Infinity);
  assert.deepEqual([...readChunk([false, null, true], 0, 3, undefined, "b").kinds], [2, 1, 2]);
});

test("chunk type checks span chunks and retain string/nullish distinctions", () => {
  const chunk = readChunk([undefined, null, "🦀", ""], 0, 4, undefined, "s");
  assert.deepEqual([...chunk.kinds], [0, 1, 4, 4]); assert.equal(chunk.strings[2], "🦀");
  assert.equal(chunk.validType, "string");
  for (const value of [false, 1, {}, 1n]) assert.throws(() => readChunk([value], 0, 1, chunk.validType, "s"), TypeError);
  assert.throws(() => readChunk(["\ud800"], 0, 1, undefined, "s"), /well-formed Unicode/);
});
