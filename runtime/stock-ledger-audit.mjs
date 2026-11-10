import { readFile, writeFile, mkdir, cp } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import { createHash } from "node:crypto";
import { performance } from "node:perf_hooks";
import { cpus } from "node:os";
import assert from "node:assert/strict";
import { build } from "./build-core.mjs";
import { createTextIO, TEXT_IO } from "./text-io.mjs";
const exec = promisify(rawExec),
  root = resolve(process.argv[2] ?? "target/stock-ledger-audit");
const source = "examples/apps/stock-ledger";
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
// Whitespace-independent source metrics; strings and comments are single tokens.
const metrics = (source) => {
  const tokens = (
    source.match(/;;[^\n]*|"(?:\\.|[^"\\])*"|[()]|[^\s()]+/gu) ?? []
  ).filter((s) => !s.startsWith(";;"));
  const heads = tokens.filter((_, i) => tokens[i - 1] === "(");
  return {
    lines: source.trimEnd().split("\n").length,
    tokens: tokens.length,
    expressions: tokens.filter((t) => t === "(").length,
    match: heads.filter((t) => t === "match").length,
    close: heads.filter((t) => t === "stream.close").length,
    resultLet: heads.filter((t) => t === "result.let").length,
    streamFold: heads.filter((t) => t === "stream.fold-result").length,
  };
};
const variants = [];
for (const name of ["main-explicit", "main"]) {
  const file = join(source, name + ".walz"),
    directory = join(root, name);
  await build(file, directory);
  const { stdout } = await exec("cargo", [
    "run",
    "--locked",
    "--quiet",
    "--",
    "ir",
    file,
  ]);
  await writeFile(join(directory, "ir.json"), stdout);
  const ir = JSON.parse(stdout),
    main = ir.functions.find((f) => f.name === "$main"),
    kinds = {};
  const walk = (node) => {
    if (node && typeof node === "object") {
      if (node.kind && node.ty) {
        const name =
          typeof node.kind === "string" ? node.kind : Object.keys(node.kind)[0];
        kinds[name] = (kinds[name] ?? 0) + 1;
      }
      for (const child of Object.values(node)) walk(child);
    }
  };
  walk(main.body);
  const { instantiate } = await import(
      pathToFileURL(join(directory, "index.js"))
    ),
    modules = new Map();
  for (const file of ["core.wasm", "memory.wasm"])
    modules.set(
      file,
      await WebAssembly.compile(await readFile(join(directory, file))),
    );
  const binary = await readFile(join(directory, "core.wasm"));
  variants.push({
    name,
    instantiate,
    modules,
    source: metrics(await readFile(file, "utf8")),
    mainIR: kinds,
    wasmBytes: binary.length,
    guestHash: hash(binary),
    scenarios: [],
  });
}
await mkdir(join(root, "source/examples/apps"), { recursive: true });
await cp(source, join(root, "source", source), { recursive: true });
await cp("walz/core", join(root, "source/walz/core"), { recursive: true });
const corpus = (unicode) => {
  const stocks = new Map(),
    lines = [],
    records = [];
  for (let i = 0; i < 3000; i++) {
    if (i % 19 === 0) {
      lines.push("# skip");
      continue;
    }
    if (i % 41 === 0) {
      lines.push("invalid");
      records.push({ ok: false, error: "invalid-command" });
      continue;
    }
    const op = ["ADD", "TAKE", "SHOW"][i % 3],
      key = unicode ? '品目"\\🦀' + (i % 31) : "key" + (i % 97),
      n = BigInt(i % 7),
      old = stocks.get(key) ?? 0n;
    lines.push(`${op}\t${key}` + (op === "SHOW" ? "" : `\t${n}`));
    if (op === "TAKE" && n > old) {
      records.push({ op, key, ok: false, error: "insufficient-stock" });
      continue;
    }
    const value = op === "ADD" ? old + n : op === "TAKE" ? old - n : old;
    if (op === "ADD" || (op === "TAKE" && stocks.has(key)))
      stocks.set(key, value);
    records.push({ op, key, ok: true, quantity: Number(value) });
  }
  return {
    bytes: new TextEncoder().encode(lines.join("\n") + "\n"),
    count: records.length,
    expected: hash(records.map((r) => JSON.stringify(r) + "\n").join("")),
  };
};
const run = async (variant, data, probe = false) => {
  let arena,
    writes = 0,
    reads = 0;
  const output = createHash("sha256");
  const { api } = await variant.instantiate({
    getCoreModule: (name) => variant.modules.get(name),
    imports: {
      [TEXT_IO]: createTextIO({
        input: (async function* () {
          for (let i = 0; i < data.bytes.length; i += 4096)
            yield data.bytes.subarray(i, i + 4096);
        })(),
        write: async (bytes) => {
          writes++;
          output.update(bytes);
        },
      }),
    },
    onTaskEvent: probe
      ? (event) => {
          if (event.type === "read-pending") reads++;
        }
      : undefined,
    instantiateCore: async (m, i) => {
      const instance = await WebAssembly.instantiate(m, i);
      if (instance.exports.alloc) arena = instance.exports;
      return instance;
    },
  });
  const start = performance.now(),
    result = await api.main(),
    milliseconds = performance.now() - start;
  assert.deepEqual(result, { tag: "ok", val: 0 });
  assert.equal(writes, data.count);
  assert.equal(output.digest("hex"), data.expected);
  assert.equal(arena.live(), 0);
  return {
    milliseconds,
    ...(probe
      ? {
          hostWrites: writes,
          jspiReads: reads,
          linearBytes: arena.memory.buffer.byteLength,
          live: arena.live(),
        }
      : {}),
  };
};
for (const unicode of [false, true]) {
  const data = corpus(unicode),
    samples = variants.map(() => []);
  for (let i = 0; i < 4; i++)
    for (const variant of variants) await run(variant, data);
  for (let i = 0; i < 9; i++)
    for (const index of i % 2 ? [1, 0] : [0, 1])
      samples[index].push((await run(variants[index], data)).milliseconds);
  variants.forEach((v, i) =>
    v.scenarios.push({
      name: unicode ? "unicode-keys" : "ascii-keys",
      rows: 3000,
      inputBytes: data.bytes.length,
      medianMs: median(samples[i]),
      samplesMs: samples[i],
      unicode,
    }),
  );
}
for (const variant of variants)
  for (const scenario of variant.scenarios) {
    const result = await run(variant, corpus(scenario.unicode), true);
    delete result.milliseconds;
    scenario.metrics = result;
  }
const report = {
  date: new Date().toISOString(),
  node: process.version,
  cpu: cpus()[0]?.model,
  method:
    "memory I/O, cached compiled modules, four warmups, nine alternating samples, SHA-256 output oracle, task counters afterwards",
  variants: variants.map(
    ({ name, source, mainIR, wasmBytes, guestHash, scenarios }) => ({
      name,
      source,
      mainIR,
      wasmBytes,
      guestHash,
      scenarios,
    }),
  ),
};
await writeFile(
  join(root, "report.json"),
  JSON.stringify(report, null, 2) + "\n",
);
console.log(JSON.stringify(report, null, 2));
