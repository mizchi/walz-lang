// Timing only: consume a frozen capture; never build/profile inside this process.
import assert from "node:assert/strict";
import { readFile, mkdir, cp, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { cpus, release } from "node:os";
import { pathToFileURL, fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";
import { load, verifySnapshot, hash, writeJson, files } from "./selfhost-bench/artifacts.mjs";
import { measure, requireEqualOutput } from "./selfhost-bench/runner.mjs";

const tick = () => new Promise(resolve_ => setImmediate(resolve_));
const median = values => [...values].sort((a, b) => a - b)[values.length >>> 1];

export async function benchmark(capture, output, options = {}) {
  assert.equal(typeof global.gc, "function", "use node --expose-gc");
  capture = resolve(capture); output = resolve(output);
  const settings = { samples: 9, warmupMs: 300, minimumWarmupCalls: 16, targetBatchMs: 10, ...options };
  assert.ok(Number.isInteger(settings.samples) && settings.samples >= 3 && settings.samples <= 99);
  await mkdir(dirname(output), { recursive: true }); await mkdir(output);
  const loaded = await load(capture); // Every live compiler must pass equality first.
  const report = { schema: "walz-selfhost-benchmark-v2", suite: loaded.manifest.suite,
    created: new Date().toISOString(), environment: { node: process.version, v8: process.versions.v8,
      platform: process.platform, osRelease: release(), arch: process.arch, cpu: cpus()[0]?.model, rustc: loaded.manifest.rustc },
    capture, captureHash: hash(await readFile(join(capture, "capture.json"))),
    method: { ...settings, variants: ["rust-native-release", "rust-wat-validate", "core", "component"],
      order: "rotate variants within each case/operation each round; no concurrent benchmarks/builds/tests",
      timing: "resident, synchronous, in-memory compiler API call; two clock reads per call; mean of calls per sample, median of sample means",
      selfhost: "Core/Component value ABI includes UTF-8 input copying, guest parsing/checking/encoding, GC and output lifting",
      native: "persistent optimized Rust process; walz::validate / walz::compile; borrowed UTF-8 input; native WAT validation fallback is recorded per case",
      watReference: "compile only: pinned wat::parse_str plus wasmparser::Validator; validates binary types/control without Walz analyses; raw output must equal full Rust compiler output",
      exclusions: "builds, process startup, module loading/instantiation, file/JSON I/O, equality/execution checks, warmup, calibration and output destruction",
      correctness: "full Wasm validation + exact canonical module bytes before timing; every measured compile output must equal its captured raw output; check results must equal function count",
      canonical: "closed unit/i32/i64 functions only; strip debug names, expand type interning, one local group per local, canonical LEB; preserve every instruction, function/local/export order and signature; fail closed on other sections/opcodes",
      gc: "one explicit Node GC outside each sampled batch; normal allocation-triggered GC remains included; microtasks drained between batches",
      limitation: "finite versioned corpus, no proof that either compiler is correct on all sources; Rust check/compile do more analysis than the current scalar selfhost" }, cases: [] };
  try {
    assert.equal(loaded.manifest.suite, "walz-selfhost-parity-v2", "recapture with the v2 references before measuring");
    for (const c of loaded.cases) for (const operation of ["check", "compile"]) {
      const variants = [{ name: "rust-native-release", sample: n => loaded.native.request({ cmd: "measure", id: c.id, operation, iterations: n }) }];
      if (operation === "compile") variants.push({ name: "rust-wat-validate", sample: n => loaded.native.request({ cmd: "measure", id: c.id, operation: "assemble", iterations: n }) });
      for (const guest of loaded.guests) {
        const variant = { run: () => guest.api[operation](c.source), verify: result => {
          assert.equal(result.tag, "ok", `${c.id}: measured compiler error`);
          if (operation === "compile") requireEqualOutput(result.val, c.outputs[guest.name]);
          else assert.equal(result.val, c.functions);
        } };
        variants.push({ name: guest.name, sample: n => measure(variant, n) });
      }
      const iterations = {}, samplesNs = {}, warmupCalls = {};
      for (const v of variants) {
        const end = performance.now() + settings.warmupMs;
        let calls = 0;
        do { await v.sample(1); calls++; await tick(); }
        while (calls < settings.minimumWarmupCalls || performance.now() < end);
        warmupCalls[v.name] = calls;
        const calibrated = await v.sample(1);
        iterations[v.name] = Math.max(1, Math.min(4096, Math.ceil(settings.targetBatchMs * 1e6 / calibrated.nsPerCall)));
        samplesNs[v.name] = [];
      }
      for (let round = 0; round < settings.samples; round++) {
        for (let offset = 0; offset < variants.length; offset++) {
          const v = variants[(round + offset) % variants.length];
          await tick(); global.gc();
          const result = await v.sample(iterations[v.name]);
          assert.equal(result.calls, iterations[v.name]);
          samplesNs[v.name].push(result.nsPerCall);
          await tick();
        }
      }
      const mediansUs = Object.fromEntries(Object.entries(samplesNs).map(([name, values]) => [name, median(values) / 1000]));
      report.cases.push({ id: c.id, operation, inputBytes: c.inputBytes, nativeBytes: c.nativeBytes, structuredCheck: c.structuredCheck,
        coreBytes: c.gate.core.bytes, componentBytes: c.gate.component.bytes, gate: c.gate,
        mediansUs, samplesNs, iterations, warmupCalls });
      console.log(c.id, operation, JSON.stringify(mediansUs));
    }
    await verifySnapshot(capture);
    // Compact public record. Full tool/runtime/source artifacts stay in capture.
    await cp(join(capture, "cases"), join(output, "cases"), { recursive: true });
    await cp(join(capture, "capture.json"), join(output, "capture.json"));
    const here = fileURLToPath(new URL(".", import.meta.url));
    await cp(join(here, "selfhost-bench"), join(output, "harness"), { recursive: true });
    await cp(fileURLToPath(import.meta.url), join(output, "selfhost-bench.mjs"));
    await cp(join(capture, "source/tools/selfhost"), join(output, "native-source/selfhost"), { recursive: true });
    await cp(join(capture, "source/tools/selfhost-bench.rs"), join(output, "native-source/selfhost-bench.rs"));
    report.recordHashes = {};
    for (const file of await files(output)) report.recordHashes[file] = hash(await readFile(join(output, file)));
    await writeJson(join(output, "results.json"), report); // Only publish success after every gate/sample.
    return report;
  } catch (error) {
    await writeFile(join(output, "failure.json"), JSON.stringify({ error: String(error), stack: error.stack }, null, 2) + "\n", { flag: "wx" });
    throw error;
  } finally { await loaded.native.close(); }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [first, second, third] = process.argv.slice(2);
  if (first === "verify" && second) {
    const checked = await load(resolve(second)); await checked.native.close();
    console.log(`Canonical equality and execution verified: ${second}`);
  } else {
    if (!first || !second) throw new Error("usage: selfhost-bench.mjs capture new-output [samples] | verify capture");
    await benchmark(first, second, third ? { samples: Number(third) } : {});
  }
}
