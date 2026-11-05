import { test } from "node:test";
import assert from "node:assert/strict";
import { summarize } from "./profile-summary.mjs";
test("guest profile summary separates self from inclusive samples and counts recursion once", () => {
  const p = {
    meta: { interval: 1 },
    threads: [
      {
        stringArray: ["guest", "a", "b", "c"],
        resourceTable: { name: [0] },
        funcTable: { name: [1, 2, 3], resource: [0, 0, 0] },
        frameTable: { func: [0, 1, 2] },
        stackTable: { frame: [0, 1, 1, 2], prefix: [null, 0, 1, 2] },
        samples: { length: 4, stack: [1, 3, 2, null], weight: [1, 2, 1, 1] },
      },
    ],
  };
  const result = summarize(p);
  assert.equal(result.samples, 4);
  assert.equal(result.self.find((row) => row.name === "guest::b").percent, 50);
  assert.equal(
    result.inclusive.find((row) => row.name === "guest::b").percent,
    100,
  );
  assert.equal(
    result.inclusive.find((row) => row.name === "guest::a").percent,
    100,
  );
});
