import test from "node:test";
import assert from "node:assert/strict";
import { corpus, suite, serializable } from "./selfhost-bench/corpus.mjs";
import { measure, requireEqualOutput } from "./selfhost-bench/runner.mjs";
import { NativePeer } from "./selfhost-bench/peer.mjs";
import { verifyExecution } from "./selfhost-bench/runner.mjs";
import { verifySnapshot, hash } from "./selfhost-bench/artifacts.mjs";
import { verifyCorpus } from "./selfhost-bench/capture.mjs";
import { mkdtemp, rm, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
const exec = promisify(callback);

test("selfhost parity corpus is deterministic and exercises scalar compiler paths", () => {
  const cases = corpus();
  assert.deepEqual(cases.map(({id, source}) => ({id, source})), corpus().map(({id, source}) => ({id, source})));
  assert.ok(cases.some(c => c.source.length > 30000));
  assert.ok(cases.some(c => c.source.includes("i64.")));
  assert.ok(cases.some(c => c.source.includes("br_if")));
  assert.ok(cases.some(c => c.probes.some(p => p.trap)));
});

test("the versioned input/probe hashes cannot silently change", async () => {
  const manifest = JSON.parse(await readFile(new URL("../benches/fixtures/selfhost-v2/manifest.json", import.meta.url)));
  assert.equal(manifest.suite, suite);
  assert.deepEqual(corpus().map(c => ({id: c.id, inputBytes: Buffer.byteLength(c.source), sourceSha256: hash(c.source), probesSha256: hash(serializable(c.probes))})), manifest.cases);
  verifyCorpus(corpus(), manifest);
  const changed = corpus(); changed[0].source += "\n";
  assert.throws(() => verifyCorpus(changed, manifest), /versioned corpus changed/);
});

test("output equality is mandatory before reporting a compiler measurement", async () => {
  assert.throws(() => requireEqualOutput(Uint8Array.of(1), Uint8Array.of(2)), /output/);
  await assert.rejects(measure({run: () => 41, verify: v => assert.equal(v, 42)}, 2), /41/);
  const r = await measure({run: () => 42, verify: v => assert.equal(v, 42)}, 3);
  assert.equal(r.calls, 3);
  assert.ok(r.nsPerCall > 0);
});

test("snapshot validation rejects modified or additional files", async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-parity-hashes-"));
  try {
    await writeFile(join(directory, "compiler.wasm"), "original");
    await writeFile(join(directory, "capture.json"), JSON.stringify({ schema: "walz-selfhost-capture-v1", hashes: { "compiler.wasm": hash("original") } }));
    await verifySnapshot(directory);
    await writeFile(join(directory, "compiler.wasm"), "changed");
    await assert.rejects(verifySnapshot(directory), /snapshot changed/);
    await writeFile(join(directory, "compiler.wasm"), "original");
    await writeFile(join(directory, "extra"), "new");
    await assert.rejects(verifySnapshot(directory), /file list changed/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("every benchmark input has identical canonical output in Core and Component", { timeout: 180000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-parity-corpus-"));
  let peer;
  try {
    await exec("cargo", ["build", "--locked", "--example", "selfhost-bench"]);
    const metadata = JSON.parse((await exec("cargo", ["metadata", "--no-deps", "--format-version", "1"])).stdout);
    peer = new NativePeer(join(metadata.target_directory, "debug/examples/selfhost-bench" + (process.platform === "win32" ? ".exe" : "")));
    const cases = [];
    for (const c of corpus()) {
      cases.push({ ...c, reference: await peer.request({ cmd: "prepare", id: c.id, source: c.source }) });
      const assembled = await peer.request({ cmd: "measure", id: c.id, operation: "assemble", iterations: 1 });
      assert.equal(assembled.calls, 1);
      assert.ok(assembled.nsPerCall > 0);
    }
    for (const backend of ["core", "component"]) {
      const output = join(directory, backend);
      await (await import(backend === "core" ? "./build-core.mjs" : "./build.mjs")).build("examples/selfhost/portable.walz", output);
      const { instantiate } = await import(pathToFileURL(join(output, "index.js")));
      const { api } = await instantiate();
      for (const c of cases) {
        const input = new TextEncoder().encode(c.source);
        assert.deepEqual(api.check(input), { tag: "ok", val: c.reference.functions }, c.id);
        const result = api.compile(input);
        assert.equal(result.tag, "ok", c.id);
        const normalized = await peer.request({ cmd: "canonical", wasm: [...result.val] });
        requireEqualOutput(normalized.canonical, c.reference.canonical);
        verifyExecution((await WebAssembly.instantiate(result.val)).instance.exports, c.probes);
      }
    }
  } finally {
    if (peer) await peer.close();
    await rm(directory, { recursive: true, force: true });
  }
});
