import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { hash, verify, measure } from "./simd-kernels/runner.mjs";
import { corpus, intersection, sum } from "./simd-kernels/workloads.mjs";

test("benchmark controls cover integer overflow, high bits and search positions", () => {
  assert.equal(sum(new Int32Array([2147483647, 2147483647, -2147483648])), 2147483646);
  assert.equal(intersection(new Int32Array([-1, -2147483648, 0]), new Int32Array([-1, -1, -1])), 33);
  for (const n of [1, 17, 4096]) {
    const cases = corpus(n);
    for (const c of cases) assert.equal(c.js(), c.expected);
    assert.equal(cases.find(c => c.name === "bytes-index-miss").expected, -1);
    assert.equal(cases.find(c => c.name === "bytes-index-late").expected, n - 1);
    assert.equal(cases.find(c => c.name === "bytes-index-early").expected, 0);
    assert.deepEqual(cases.map(c => Array.from(c.a)), corpus(n).map(c => Array.from(c.a)));
  }
});
test("benchmark rejects wrong answers and mutated frozen artifacts", async () => {
  assert.throws(() => measure({ broken: () => 41 }, 42, 5, 1), /reference mismatch/);
  const dir = await mkdtemp(join(tmpdir(), "walz-simd-freeze-"));
  try {
    await writeFile(join(dir, "kernel"), "original");
    await writeFile(join(dir, "capture.json"), JSON.stringify({ hashes: { kernel: hash("original") } }));
    await verify(dir);
    await writeFile(join(dir, "kernel"), "changed");
    await assert.rejects(verify(dir), /snapshot changed/);
  } finally { await rm(dir, { recursive: true, force: true }); }
});
