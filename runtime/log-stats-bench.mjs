// Compare saved production artifacts. Compile/setup and instrumented allocation
// probes are excluded from all latency samples; output JSON is checked each time.
import { readFile, writeFile, mkdtemp, rm } from "node:fs/promises";
import { resolve, join } from "node:path";
import { tmpdir, cpus } from "node:os";
import { pathToFileURL } from "node:url";
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import { performance } from "node:perf_hooks";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { createTextIO, TEXT_IO } from "./text-io.mjs";
const exec = promisify(rawExec);
const directories = process.argv.slice(2).map((x) => resolve(x));
if (directories.length < 2 || directories.length > 3)
  throw new Error("usage: log-stats-bench.mjs before after [candidate]");
const temporary = await mkdtemp(join(tmpdir(), "wlt-arena-probe-"));
try {
  const arena = await readFile(
    new URL("../src/component-memory.wat", import.meta.url),
    "utf8",
  );
  const marker = '(func $alloc (export "alloc")';
  assert.equal(arena.split(marker).length, 2);
  const instrumented = `(module ${arena.replace(marker, "(func $arena_alloc")}
    (global $calls (mut i64) (i64.const 0)) (global $bytes (mut i64) (i64.const 0))
    (func $alloc (export "alloc") (param $n i32) (result i32) (local $p i32)
      (local.set $p (call $arena_alloc (local.get $n)))
      (global.set $calls (i64.add (global.get $calls) (i64.const 1)))
      (global.set $bytes (i64.add (global.get $bytes) (i64.extend_i32_u (local.get $n)))) (local.get $p))
    (func (export "alloc-count") (result i64) (global.get $calls))
    (func (export "alloc-bytes") (result i64) (global.get $bytes)))`;
  await writeFile(join(temporary, "arena.wat"), instrumented);
  await exec("wasm-tools", [
    "parse",
    join(temporary, "arena.wat"),
    "-o",
    join(temporary, "arena.wasm"),
  ]);
  const probeModule = await WebAssembly.compile(
    await readFile(join(temporary, "arena.wasm")),
  );
  await writeFile(join(temporary, "original.wat"), `(module ${arena})`);
  await exec("wasm-tools", [
    "parse",
    join(temporary, "original.wat"),
    "-o",
    join(temporary, "original.wasm"),
  ]);
  const originalArena = await readFile(join(temporary, "original.wasm"));
  const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
  const variants = [];
  for (const directory of directories) {
    const guest = await readFile(join(directory, "core.wasm"));
    const memory = await readFile(join(directory, "memory.wasm"));
    const captured = JSON.parse(
      await readFile(join(directory, "report.json"), "utf8"),
    );
    assert.equal(hash(guest), captured.coreHash, "guest changed after capture");
    assert.ok(
      memory.equals(originalArena),
      "allocation probe requires the same allocator as the saved production artifact",
    );
    const { instantiate } = await import(
      pathToFileURL(join(directory, "index.js"))
    );
    const modules = new Map();
    for (const name of ["core.wasm", "memory.wasm"])
      modules.set(
        name,
        await WebAssembly.compile(await readFile(join(directory, name))),
      );
    variants.push({
      directory,
      coreHash: hash(guest),
      memoryHash: hash(memory),
      instantiate,
      modules,
      scenarios: [],
    });
  }
  const configurations = [
    {
      name: "short-ascii",
      rows: 16000,
      message: "x".repeat(32),
      chunk: 4096,
      invalidEvery: 0,
    },
    {
      name: "unicode-errors",
      rows: 16000,
      message: "日本語🦀".repeat(4),
      chunk: 4096,
      invalidEvery: 4,
    },
    {
      name: "long-ascii",
      rows: 1000,
      message: "x".repeat(4096),
      chunk: 4096,
      invalidEvery: 0,
    },
    {
      name: "small-chunks-errors",
      rows: 16000,
      message: "x".repeat(64),
      chunk: 64,
      invalidEvery: 2,
    },
  ];
  const corpus = (config) => {
    const levels = Object.fromEntries(
      ["INFO", "WARN", "ERROR"].map((name) => [
        name,
        { count: 0, total_ms: 0, max_ms: 0 },
      ]),
    );
    const expected = { accepted: 0, rejected: 0, ignored: 0, levels };
    const lines = Array.from({ length: config.rows }, (_, i) => {
      if (config.invalidEvery && i % config.invalidEvery === 0) {
        expected.rejected++;
        return "invalid\t-1\t" + config.message;
      }
      expected.accepted++;
      const name = Object.keys(levels)[i % 3],
        ms = i % 1000,
        bucket = levels[name];
      bucket.count++;
      bucket.total_ms += ms;
      bucket.max_ms = Math.max(bucket.max_ms, ms);
      return `${name}\t${ms}\t${config.message}`;
    });
    return {
      bytes: new TextEncoder().encode(lines.join("\n") + "\n"),
      expected,
    };
  };
  const run = async (variant, config, data, probe = false) => {
    const output = [];
    let memory,
      reads = 0,
      writes = 0;
    const { api } = await variant.instantiate({
      getCoreModule: (name) =>
        probe && name === "memory.wasm"
          ? probeModule
          : variant.modules.get(name),
      imports: {
        [TEXT_IO]: createTextIO({
          input: (async function* () {
            for (let i = 0; i < data.bytes.length; i += config.chunk)
              yield data.bytes.subarray(i, i + config.chunk);
          })(),
          write: async (bytes) => {
            writes++;
            output.push(Buffer.from(bytes));
          },
        }),
      },
      onTaskEvent: probe
        ? (event) => {
            if (event.type === "read-pending") reads++;
          }
        : undefined,
      instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.alloc) memory = instance.exports;
        return instance;
      },
    });
    const start = performance.now();
    const result = await api.main();
    const milliseconds = performance.now() - start;
    assert.deepEqual(result, { tag: "ok", val: 0 });
    assert.deepEqual(
      JSON.parse(Buffer.concat(output).toString()),
      data.expected,
    );
    assert.equal(memory.live(), 0);
    return {
      milliseconds,
      linearBytes: memory.memory.buffer.byteLength,
      live: memory.live(),
      ...(probe
        ? {
            allocations: Number(memory["alloc-count"]()),
            requestedBytes: Number(memory["alloc-bytes"]()),
            jspiReads: reads,
            hostWrites: writes,
          }
        : {}),
    };
  };
  for (const config of configurations) {
    const data = corpus(config),
      samples = variants.map(() => []);
    for (let i = 0; i < 4; i++)
      for (const variant of variants) await run(variant, config, data);
    for (let i = 0; i < 9; i++) {
      const order = variants.map((_, j) => (j + i) % variants.length);
      if (i % 2) order.reverse();
      for (const index of order)
        samples[index].push(
          (await run(variants[index], config, data)).milliseconds,
        );
    }
    variants.forEach((variant, i) =>
      variant.scenarios.push({
        ...config,
        inputBytes: data.bytes.length,
        samplesMs: samples[i],
        medianMs: [...samples[i]].sort((a, b) => a - b)[4],
      }),
    );
  }
  for (const variant of variants)
    for (const scenario of variant.scenarios) {
      const metrics = await run(variant, scenario, corpus(scenario), true);
      delete metrics.milliseconds;
      scenario.metrics = metrics;
    }
  const report = {
    date: new Date().toISOString(),
    node: process.version,
    cpu: cpus()[0]?.model,
    method:
      "memory I/O; cached compiled modules; four warmups; nine rotated/interleaved samples; allocation probes afterwards",
    variants: variants.map(
      ({ directory, coreHash, memoryHash, scenarios }) => ({
        directory,
        coreHash,
        memoryHash,
        scenarios,
      }),
    ),
  };
  await writeFile(
    join(directories.at(-1), "comparison.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await rm(temporary, { recursive: true, force: true });
}
