// Save production modules before editing; compare frozen inputs in alternating order.
import { mkdir, readFile, writeFile, readdir, copyFile } from "node:fs/promises";
import { join, resolve, relative } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { performance } from "node:perf_hooks";
import { cpus } from "node:os";
import assert from "node:assert/strict";
import { build } from "./build-core.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (path, data) => writeFile(path, JSON.stringify(data, null, 2) + "\n");
async function files(directory) {
  const out = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    out.push(...(entry.isDirectory() ? await files(path) : [path]));
  }
  return out.sort();
}
function corpus() {
  const n = 600, key = '品目"\\🦀';
  return {
    "log-stats": {
      lines: Array.from({ length: n }, () => ["INFO\t1\tx", "WARN\t2\t日本語", "ERROR\t3\tx"]).flat(),
      expected: [{ accepted: 3 * n, rejected: 0, ignored: 0, levels: {
        INFO: { count: n, total_ms: n, max_ms: 1 },
        WARN: { count: n, total_ms: 2 * n, max_ms: 2 },
        ERROR: { count: n, total_ms: 3 * n, max_ms: 3 },
      } }],
      result: 0,
    },
    "stock-ledger": {
      lines: Array.from({ length: n }, () => [`ADD	${key}	1`, `TAKE	${key}	1`, `SHOW	${key}`]).flat(),
      expected: Array.from({ length: n }, () => [
        { op: "ADD", key, ok: true, quantity: 1 },
        { op: "TAKE", key, ok: true, quantity: 0 },
        { op: "SHOW", key, ok: true, quantity: 0 },
      ]).flat(),
      result: 0,
    },
    workflow: {
      lines: ["TASK\ta\t1", "TASK\tb\t2", ...Array(n).fill("AFTER\tb\ta"), "PLAN\t2"],
      expected: [
        { line: n + 3, ok: true, kind: "plan", workers: 2, tasks: 2, makespan: 3 },
        { line: n + 3, kind: "job", id: "a", worker: 0, start: 0, finish: 1 },
        { line: n + 3, kind: "job", id: "b", worker: 0, start: 1, finish: 3 },
      ],
      result: n + 3,
    },
  };
}
async function capture(directory) {
  await mkdir(resolve(directory, ".."), { recursive: true });
  await mkdir(directory); // Never overwrite a snapshot.
  await save(join(directory, "corpus.json"), corpus());
  await copyFile(join(root, "runtime/text-io.mjs"), join(directory, "text-io.mjs"));
  for (const name of Object.keys(corpus())) {
    await build(join(root, `examples/apps/${name}/main.walz`), join(directory, name));
    for (const folder of [`examples/apps/${name}`, "walz/core"]) {
      for (const path of await files(join(root, folder))) {
        if (!path.endsWith(".walz")) continue;
        const target = join(directory, "source", relative(root, path));
        await mkdir(resolve(target, ".."), { recursive: true });
        await copyFile(path, target);
      }
    }
  }
  const hashes = {};
  for (const path of await files(directory)) hashes[relative(directory, path)] = hash(await readFile(path));
  await save(join(directory, "capture.json"), { node: process.version, cpu: cpus()[0]?.model, hashes });
  console.log(directory);
}
async function verify(directory) {
  const manifest = JSON.parse(await readFile(join(directory, "capture.json")));
  for (const [path, expected] of Object.entries(manifest.hashes))
    assert.equal(hash(await readFile(join(directory, path))), expected, path);
  return manifest;
}
async function runner(directory, name, c) {
  const { createTextIO, TEXT_IO } = await import(pathToFileURL(join(directory, "text-io.mjs")));
  const { instantiate } = await import(pathToFileURL(join(directory, name, "index.js")));
  const bytes = new TextEncoder().encode(c.lines.join("\n") + "\n");
  let records, closed, arena;
  const { api } = await instantiate({
    imports: { [TEXT_IO]: {
      readLines() { return createTextIO({
        input: (async function* () { try { yield bytes; } finally { closed++; } })(),
        write: async (bytes) => records.push(JSON.parse(new TextDecoder().decode(bytes))),
      }).readLines(); },
      writeLine(line) { records.push(JSON.parse(line)); return Promise.resolve({ tag: "ok" }); },
    } },
    instantiateCore: async (m, i) => {
      const instance = await WebAssembly.instantiate(m, i);
      if (instance.exports.alloc) arena = instance.exports;
      return instance;
    },
  });
  return async () => {
    records = []; closed = 0;
    const start = performance.now(), result = await api.main(), ms = performance.now() - start;
    assert.deepEqual(result, { tag: "ok", val: c.result });
    assert.deepEqual(records, c.expected);
    assert.equal(closed, 1); assert.equal(arena.live(), 0);
    return ms;
  };
}
async function compare(before, after, rounds) {
  assert.ok(Number.isInteger(rounds) && rounds >= 3 && rounds <= 15);
  const manifests = await Promise.all([verify(before), verify(after)]);
  for (const name of ["corpus.json", "text-io.mjs"])
    assert.equal(manifests[0].hashes[name], manifests[1].hashes[name]);
  const cases = JSON.parse(await readFile(join(before, "corpus.json"))), results = {};
  for (const [name, c] of Object.entries(cases)) {
    const runs = await Promise.all([before, after].map((d) => runner(d, name, c)));
    for (const run of runs) for (let i = 0; i < 3; i++) await run();
    const samples = [[], []];
    for (let round = 0; round < rounds; round++)
      for (const i of round % 2 ? [1, 0] : [0, 1]) {
        let total = 0;
        for (let batch = 0; batch < 10; batch++) total += await runs[i]();
        samples[i].push(total / 10);
      }
    const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
    results[name] = {
      samples, medianBeforeMs: median(samples[0]), medianAfterMs: median(samples[1]),
      wasmBytes: await Promise.all([before, after].map(async (d) => (await readFile(join(d, name, "core.wasm"))).length)),
    };
  }
  const report = { before, after, rounds, callsPerSample: 10, node: process.version, cpu: cpus()[0]?.model, results };
  await save(join(after, "comparison.json"), report);
  console.log(JSON.stringify(report, null, 2));
}
const [mode, first, second, rounds = "5"] = process.argv.slice(2);
if (mode === "capture" && first && !second) await capture(resolve(first));
else if (mode === "compare" && first && second) await compare(resolve(first), resolve(second), Number(rounds));
else throw new Error("usage: stdlib-bench.mjs capture directory | compare before after [rounds]");
