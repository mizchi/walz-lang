import test from "node:test";
import assert from "node:assert/strict";
import { program } from "./testing/table-wire.mjs";
import { op } from "./table/index.mjs";

test("table expressions resolve aggregate namespace aliases after minification", () => {
  const a = op;
  const result = program(d => a.sum(d.x * 2), ["x"], { aggregate: true });
  assert.equal(result.nodes[result.root].op, 101);
  assert.equal(result.nodes[result.nodes[result.root].a].op, 12);
});
test("table expression validation does not execute host functions or accept prototype methods", () => {
  let called = false;
  const host = () => { called = true; return 1; };
  for (const expression of [d => host(d.x), d => op.toString(d.x)]) {
    assert.throws(() => program(expression, ["x"], { aggregate: true }), TypeError);
  }
  assert.equal(called, false);
  assert.throws(() => program(d => op.sum(missing), ["x"], { aggregate: true }), ReferenceError);
  assert.throws(() => program(d => op.sum(d.x), ["x"]), TypeError);
  assert.throws(() => program(d => d.x, ["x"], { aggregate: true }), TypeError);
});
