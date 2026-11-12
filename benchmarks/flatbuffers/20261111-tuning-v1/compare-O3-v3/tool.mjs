import assert from "node:assert/strict";
import { readFile, writeFile, mkdir, copyFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";
import { cpus } from "node:os";
import { verify } from "./flatbuffers-bench/runner.mjs";
import { hash } from "./flatbuffers-bench/workloads.mjs";
import { createJobs } from "./flatbuffers-inspect.mjs";

const median = xs => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
export function assertComparable(before, after) {
  assert.deepEqual(before.reference, after.reference, "reference implementation changed");
  // This paired driver never executes capture/runner.mjs; its version can differ.
  const input = name => name === "corpus.json" || name === "reference.mjs" || name === "workloads.mjs" || name.startsWith("inputs/") || name.startsWith("reference-package/");
  const names = Object.keys(before.hashes).filter(input);
  assert.ok(names.includes("corpus.json"), "missing corpus provenance");
  for (const name of names) assert.equal(before.hashes[name], after.hashes[name], `comparison input changed: ${name}`);
  assert.deepEqual(names.sort(), Object.keys(after.hashes).filter(input).sort());
}
let sink;
export async function measureBatch(job, calls) {
  const start = performance.now(); for (let i = 0; i < calls; i++) sink = job.fn();
  const ns = (performance.now() - start) * 1e6;
  job.check(sink);
  // Synchronous exports can enqueue fulfilled Task promises. Let their jobs run
  // before the next batch; otherwise warmup retains their values until OOM.
  await new Promise(resolve => setImmediate(resolve));
  job.clean?.(); return ns / calls;
}
export async function compare(before, after, output, { filter = "", rounds = 9, targetMs = 20 } = {}) {
  assert.equal(typeof global.gc, "function", "run with node --expose-gc");
  assert.ok(Number.isInteger(rounds) && rounds >= 5 && rounds <= 31);
  const manifests = await Promise.all([verify(before), verify(after)]);
  assertComparable(...manifests);
  const selected = (await Promise.all([createJobs(before), createJobs(after)])).map(jobs => jobs.filter(j => !j.name.endsWith(".js") && j.name.includes(filter)));
  assert.deepEqual(selected[0].map(j => j.name), selected[1].map(j => j.name));
  assert.ok(selected[0].length, "no comparison cases matched");
  await mkdir(output);
  await copyFile(fileURLToPath(import.meta.url), join(output, "tool.mjs"));
  await copyFile(new URL("./flatbuffers-inspect.mjs", import.meta.url), join(output, "jobs.mjs"));
  const report = { schema: "walz-flatbuffers-paired-v3", created: new Date().toISOString(), node: process.version, v8: process.versions.v8,
    cpu: cpus()[0]?.model, before: hash(await readFile(join(before, "capture.json"))), after: hash(await readFile(join(after, "capture.json"))),
    capturedRunnersEqual: manifests[0].hashes["runner.mjs"] === manifests[1].hashes["runner.mjs"], capturedRunnersExecuted: false,
    method: "Same inputs and reference code. Warmup at least 4096 calls and 250ms per side before calibration; then calibrate on before and use the same iteration count for after. Alternate before/after order; GC before each side; automatic GC counts. Yield the event loop after every batch to drain fulfilled Task promises. Equality, microtask draining and cleanup checks are outside clocks. No profiler attached.", rounds, targetMs, cases: [] };
  for (let index = 0; index < selected[0].length; index++) {
    const jobs = selected.map(side => side[index]);
    for (const job of jobs) { job.check(job.fn()); job.clean?.(); }
    for (const job of jobs) {
      const start = performance.now(); let warmed = 0;
      do { await measureBatch(job, 64); warmed += 64; } while (warmed < 4096 || performance.now() - start < 250);
    }
    let calls = 1;
    while (calls < 1 << 20 && (await measureBatch(jobs[0], calls)) * calls < targetMs * 1e6) calls *= 2;
    const samples = [[], []];
    for (let round = 0; round < rounds; round++) for (const side of round % 2 ? [1, 0] : [0, 1]) {
      global.gc(); samples[side].push(await measureBatch(jobs[side], calls));
    }
    const result = { name: jobs[0].name, calls, beforeNs: samples[0], afterNs: samples[1], medianBeforeNs: median(samples[0]), medianAfterNs: median(samples[1]), speedup: median(samples[0].map((n, i) => n / samples[1][i])) };
    report.cases.push(result);
    console.log(`${result.name}: ${(result.medianBeforeNs / 1000).toFixed(2)} → ${(result.medianAfterNs / 1000).toFixed(2)} µs, ${result.speedup.toFixed(2)}×`);
  }
  await writeFile(join(output, "comparison.json"), JSON.stringify(report, null, 2) + "\n", { flag: "wx" });
  return report;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [before, after, output, filter] = process.argv.slice(2);
  if (!before || !after || !output) throw new Error("usage: flatbuffers-compare.mjs before-capture after-capture output-directory [filter]");
  await compare(resolve(before), resolve(after), resolve(output), { filter });
}
