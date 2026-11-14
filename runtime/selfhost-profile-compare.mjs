// Compare the same source with two independent compiler artifacts. Timing ratios
// are published only when both succeed and emitted binaries agree exactly.
import assert from "node:assert/strict";
import { writeFile, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { readProject } from "./selfhost/files.mjs";
import { openCompiler, sourceRecords, recorderRecords, describe, sha256 } from "./selfhost/profiling.mjs";

const [beforePath, afterPath, input, output, count = "9"] = process.argv.slice(2);
const samples = Number(count);
if (!beforePath || !afterPath || !input || !output || !Number.isInteger(samples) || samples < 3 || samples > 99)
  throw new Error("usage: selfhost-profile-compare.mjs before-compiler after-compiler input.walz output.json [samples:3..99]");
const before = await openCompiler(beforePath), after = await openCompiler(afterPath);
const left = await readProject(before.api, input), right = await readProject(after.api, input);
assert.deepEqual(sourceRecords(left.files), sourceRecords(right.files), "dependency discovery or inputs differ");
assert.ifError(left.error && new Error(JSON.stringify(left.error)));
assert.ifError(right.error && new Error(JSON.stringify(right.error)));
const states = [before, after];
for (const state of states) {
  state.profile = state.api.profileProject(left.files); // Untimed warm-up.
  state.times = [];
  if (state.profile.result.tag === "ok") {
    state.binary = state.api.compileProject(left.files);
    assert.equal(state.binary.tag, "ok", "profiling must preserve acceptance");
    assert.ok(WebAssembly.validate(state.binary.val));
    state.binaryHash = sha256(state.binary.val);
  }
}
const matched = before.profile.result.tag === "ok" && after.profile.result.tag === "ok" && before.binaryHash === after.binaryHash;
for (let round = 0; matched && round < samples; round++) {
  for (const index of round % 2 ? [1, 0] : [0, 1]) {
    const state = states[index], start = performance.now();
    const profile = state.api.profileProject(left.files);
    state.times.push(performance.now() - start);
    assert.deepEqual(profile, state.profile, "each request must produce deterministic isolated counters");
  }
}
const median = (values) => {
  const sorted = values.toSorted((a, b) => a - b), middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
};
const record = (state) => ({
  compiler: state.artifacts,
  ...state.profile,
  ...describe(state.profile.stats),
  binarySha256: state.binaryHash ?? null,
  elapsedMs: state.times,
  medianMs: state.times.length ? median(state.times) : null,
});
const report = {
    schema: "walz-selfhost-profile-comparison-v4",
  recorder: await recorderRecords(import.meta.url),
  environment: { node: process.version, v8: process.versions.v8, platform: process.platform, arch: process.arch },
  sources: sourceRecords(left.files), requestedSamples: samples, timedSamples: matched ? samples : 0,
  binaryEquality: matched,
  speedup: matched ? median(before.times) / median(after.times) : null,
  before: record(before), after: record(after),
};
await mkdir(dirname(resolve(output)), { recursive: true });
await writeFile(output, JSON.stringify(report, null, 2) + "\n");
console.log(matched ? `Identical Wasm; ${report.speedup.toFixed(2)}x; ${output}` : `Outcomes recorded; timing ratio withheld; ${output}`);
if (before.profile.result.tag === "ok" && after.profile.result.tag === "ok" && !matched) process.exitCode = 1;
