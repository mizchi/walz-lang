import test from "node:test";
import assert from "node:assert/strict";
import { materialize } from "./table/materialize.mjs";

test("row materialization preserves own properties, descriptors, prototypes and values", () => {
  for (const count of [0, 1, 2, 3, 4, 8, 17]) {
    const names = ["__proto__", "constructor", "0", "2", "", "日本🦀", ...Array.from({ length: 17 }, (_, i) => `c${i}`)].slice(0, count);
    const columns = names.map((_, c) => [undefined, null, NaN, -0, Infinity, c]);
    const expected = Array.from({ length: 6 }, (_, row) => Object.fromEntries(names.map((name, c) => [name, columns[c][row]])));
    const actual = materialize(names, columns, 6);
    assert.deepEqual(actual, expected);
    for (const row of actual) {
      assert.equal(Object.getPrototypeOf(row), Object.prototype);
      for (const name of names) assert.deepEqual(Object.getOwnPropertyDescriptor(row, name),
        { value: row[name], writable: true, enumerable: true, configurable: true });
    }
    if (names.length) { actual[0][names[0]] = 42; assert.equal(actual[1][names[0]], null); assert.equal(columns[0][0], undefined); }
    assert.deepEqual(materialize(names, columns, 0), []);
  }
});

test("materialization bypasses inherited setters without executing generated code", () => {
  let called = 0;
  Object.defineProperty(Object.prototype, "walzTestSetter", { configurable: true, set() { called++; } });
  try {
    const result = materialize(["walzTestSetter", "__proto__"], [[1, 2], ["x", "y"]], 2);
    assert.equal(called, 0); assert.equal(result[0].walzTestSetter, 1); assert.equal(result[1]["__proto__"], "y");
  } finally { delete Object.prototype.walzTestSetter; }
});
