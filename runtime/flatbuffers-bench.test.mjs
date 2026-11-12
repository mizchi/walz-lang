import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import * as flatbuffers from "flatbuffers";
import { corpus, materialize, hash } from "./flatbuffers-bench/workloads.mjs";
import { createReference } from "./flatbuffers-bench/reference.mjs";
import { measure, verify } from "./flatbuffers-bench/runner.mjs";

test("FlatBuffers benchmark fixes inputs covering Unicode, bytes and numeric vectors", () => {
  const cases = corpus();
  assert.deepEqual(cases, corpus());
  assert.equal(cases.length, 4);
  for (const c of cases) assert.equal(c.inputHash, hash(JSON.stringify(c.value)));
  assert.ok(cases.some(c => c.value.payload.length >= 65536));
  assert.ok(cases.some(c => c.value.values.length >= 16384));
  assert.ok(cases.some(c => c.value.name.includes("日本語")));
});

test("official reference materializes every field and returns independently owned encode/decode buffers", () => {
  const reference = createReference(flatbuffers), reused = reference.createEncoder();
  for (const c of corpus()) {
    const value = materialize(c.value);
    const bytes = reference.encode(value), saved = bytes.slice();
    const decoded = reference.decode(bytes);
    assert.deepEqual(decoded, value);
    const previous = reused(value), expected = previous.slice();
    reused(materialize(corpus()[0].value));
    assert.deepEqual(previous, expected, "reusing a builder cannot mutate previous output");
    decoded.payload.fill(0); decoded.values.fill(0);
    assert.deepEqual(bytes, saved, "materialized vectors cannot alias input bytes");
    assert.deepEqual(value, materialize(c.value), "encoding cannot mutate the input");
  }
});

test("FlatBuffers benchmark rejects changed captures and incorrect measured implementations", async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-flatbuffers-bench-"));
  try {
    await writeFile(join(directory, "input.bin"), "valid");
    await writeFile(join(directory, "capture.json"), JSON.stringify({ schema: "walz-flatbuffers-capture-v1", hashes: { "input.bin": hash("valid") } }));
    await verify(directory);
    await writeFile(join(directory, "input.bin"), "changed");
    await assert.rejects(verify(directory), /snapshot changed/);
  } finally { await rm(directory, { force: true, recursive: true }); }
  await assert.rejects(measure({ broken: { fn: () => 41, check: value => assert.equal(value, 42) } }, { rounds: 5, targetMs: 1 }), /41/);
});

test("FlatBuffers comparison reports raw samples with a rotating measurement order", async () => {
  let calls = 0;
  const results = await measure({ identity: { fn: () => ++calls, check: value => assert.ok(value > 0) } }, { rounds: 5, targetMs: 1 });
  assert.equal(results.identity.samplesNs.length, 5);
  assert.ok(results.identity.callsPerSample > 0);
  assert.ok(results.identity.medianNs > 0);
});

test("current benchmark drains fulfilled promise jobs between measured batches", async () => {
  let pending = 0;
  await measure({ sync: { fn() { pending++; Promise.resolve().then(() => pending--); return 42; },
    check(value) { assert.equal(value, 42); }, clean() { assert.equal(pending, 0); } } }, { rounds: 5, targetMs: 1 });
  assert.equal(pending, 0);
});
