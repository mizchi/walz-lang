// Frozen production artifacts, paired latency samples, CPU/timer evidence and
// a native guest profile. Output and ABI cleanup are checked on every call.
import {
  mkdir,
  readFile,
  writeFile,
  readdir,
  copyFile,
} from "node:fs/promises";
import { resolve, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import { createHash } from "node:crypto";
import { cpus } from "node:os";
import { Session } from "node:inspector";
import assert from "node:assert/strict";
import { summarize } from "./profile-summary.mjs";
const exec = promisify(rawExec),
  root = fileURLToPath(new URL("..", import.meta.url));
const hash = (data) => createHash("sha256").update(data).digest("hex");
const json = async (path) => JSON.parse(await readFile(path));
async function files(directory) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    output.push(...(entry.isDirectory() ? await files(path) : [path]));
  }
  return output.sort();
}
async function save(path, data) {
  await writeFile(path, JSON.stringify(data, null, 2) + "\n");
}

async function capture(directory) {
  await mkdir(resolve(directory, ".."), { recursive: true });
  await mkdir(directory); // Refuse to replace a previous snapshot.
  for (const backend of ["core", "component"]) {
    const { build } = await import(
      backend === "core" ? "./build-core.mjs" : "./build.mjs"
    );
    for (const [kind, source] of [
      ["io", "main"],
      ["compute", "benchmark"],
    ])
      await build(
        join(root, `examples/apps/workflow/${source}.wlt`),
        join(directory, `${backend}-${kind}`),
      );
  }
  await exec(
    "cargo",
    [
      "run",
      "--locked",
      "--quiet",
      "--",
      "build",
      "examples/apps/workflow/benchmark.wlt",
      join(directory, "compute.wasm"),
    ],
    { cwd: root },
  );
  for (const name of ["text-io.mjs", "workflow-oracle.mjs"])
    await copyFile(join(root, "runtime", name), join(directory, name));
  for (const folder of ["examples/apps/workflow", "stdlib"]) {
    for (const name of await readdir(join(root, folder))) {
      if (!name.endsWith(".wlt")) continue;
      const target = join(directory, "source", folder, name);
      await mkdir(resolve(target, ".."), { recursive: true });
      await copyFile(join(root, folder, name), target);
    }
  }
  const hashes = {};
  for (const file of await files(directory))
    hashes[relative(directory, file)] = hash(await readFile(file));
  await save(join(directory, "capture.json"), {
    schema: "workflow-bench-v1",
    node: process.version,
    cpu: cpus()[0]?.model,
    hashes,
  });
  console.log(directory);
}
async function verify(directory) {
  const captured = await json(join(directory, "capture.json"));
  for (const [name, expected] of Object.entries(captured.hashes))
    assert.equal(
      hash(await readFile(join(directory, name))),
      expected,
      `snapshot changed: ${directory}/${name}`,
    );
  return captured;
}
const ioCommands = (rows) => [
  "TASK\ta\t1",
  "TASK\tb\t2",
  ...Array.from({ length: rows }, () => "AFTER\tb\ta"),
  "PLAN\t2",
];
const chain = (n) => [
  ...Array.from({ length: n }, (_, i) => `TASK	c${i}	1000000`),
  ...Array.from({ length: n - 1 }, (_, i) => `AFTER	c${i + 1}	c${i}`),
  "PLAN\t7",
];

async function runner(directory, backend, lines) {
  const { createTextIO, TEXT_IO } = await import(
    pathToFileURL(join(directory, "text-io.mjs"))
  );
  const { workflowOracle } = await import(
    pathToFileURL(join(directory, "workflow-oracle.mjs"))
  );
  const bytes = new TextEncoder().encode(lines.join("\n") + "\n"),
    expected = workflowOracle(lines);
  let io, records, closed, arena;
  const runtime = await import(
    pathToFileURL(join(directory, `${backend}-io/index.js`))
  );
  const program = await runtime.instantiate({
    imports: {
      [TEXT_IO]: {
        readLines() {
          io = createTextIO({
            input: (async function* () {
              try {
                yield bytes;
              } finally {
                closed++;
              }
            })(),
            write: async (bytes) =>
              records.push(JSON.parse(new TextDecoder().decode(bytes))),
          });
          return io.readLines();
        },
        writeLine(line) {
          return io.writeLine(line);
        },
      },
    },
    instantiateCore: async (m, i) => {
      const instance = await WebAssembly.instantiate(m, i);
      if (instance.exports.alloc) arena = instance.exports;
      return instance;
    },
  });
  return async () => {
    records = [];
    closed = 0;
    const start = performance.now(),
      value = await program.api.main(),
      ms = performance.now() - start;
    assert.deepEqual(value, { tag: "ok", val: lines.length });
    assert.deepEqual(records, expected);
    assert.equal(closed, 1);
    assert.equal(arena.live(), 0);
    return ms;
  };
}
async function computeRunner(directory, backend, kind) {
  const runtime = await import(
    pathToFileURL(join(directory, `${backend}-compute/index.js`))
  );
  const program = await runtime.instantiate();
  return async () => {
    const start = performance.now();
    const result =
      kind === "chain"
        ? await program.api.chain(2200, 1)
        : await program.api.edgeLimit();
    const ms = performance.now() - start;
    assert.equal(result, kind === "chain" ? 2200000000n : 1);
    return ms;
  };
}
async function compare(before, after, rounds) {
  assert.ok(Number.isInteger(rounds) && rounds >= 3 && rounds <= 9);
  const manifests = await Promise.all([verify(before), verify(after)]);
  assert.equal(
    manifests[0].hashes["workflow-oracle.mjs"],
    manifests[1].hashes["workflow-oracle.mjs"],
  );
  const results = {};
  for (const backend of ["core", "component"]) {
    results[backend] = {};
    for (const kind of [
      "io-commands",
      "io-chain",
      "compute-chain",
      "compute-edges",
    ]) {
      const runs = await Promise.all(
        [before, after].map((directory) =>
          kind.startsWith("io-")
            ? runner(
                directory,
                backend,
                kind === "io-chain" ? chain(2200) : ioCommands(3000),
              )
            : computeRunner(
                directory,
                backend,
                kind === "compute-chain" ? "chain" : "edges",
              ),
        ),
      );
      // One unmeasured call warms each saved implementation. Alternate order
      // by round so all before samples do not precede all after samples.
      for (const run of runs) await run();
      const samples = [[], []];
      for (let round = 0; round < rounds; round++)
        for (const index of round % 2 ? [1, 0] : [0, 1])
          samples[index].push(await runs[index]());
      const median = (values) =>
        [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
      const medians = samples.map(median);
      results[backend][kind] = {
        before: samples[0],
        after: samples[1],
        medianBeforeMs: medians[0],
        medianAfterMs: medians[1],
        speedup: medians[0] / medians[1],
      };
      console.log(backend, kind, JSON.stringify(results[backend][kind]));
    }
  }
  await save(join(after, "comparison.json"), {
    before,
    after,
    rounds,
    node: process.version,
    cpu: cpus()[0]?.model,
    results,
  });
}
async function profile(directory) {
  await verify(directory);
  const run = await runner(directory, "component", ioCommands(2000));
  const session = new Session();
  session.connect();
  const post = promisify(session.post.bind(session));
  await post("Profiler.enable");
  await post("Profiler.start");
  const nativeTimer = globalThis.setTimeout,
    timers = {};
  globalThis.setTimeout = (fn, delay, ...args) => {
    timers[delay ?? 0] = (timers[delay ?? 0] ?? 0) + 1;
    return nativeTimer(fn, delay, ...args);
  };
  let elapsedMs, cpu;
  try {
    elapsedMs = await run();
  } finally {
    globalThis.setTimeout = nativeTimer;
    cpu = (await post("Profiler.stop")).profile;
    session.disconnect();
  }
  await save(join(directory, "component.cpuprofile"), cpu);
  const nodes = new Map(cpu.nodes.map((node) => [node.id, node])),
    self = new Map();
  for (let i = 0; i < cpu.samples.length; i++) {
    const name = nodes.get(cpu.samples[i]).callFrame.functionName;
    self.set(name, (self.get(name) ?? 0) + cpu.timeDeltas[i] / 1000);
  }
  const native = await exec(
    "wasmtime",
    [
      "run",
      "-W",
      "gc=y,exceptions=y,function-references=y",
      `--profile=guest,${join(directory, "guest.json")},1ms`,
      "--invoke",
      "chain",
      join(directory, "compute.wasm"),
      "4096",
      "20",
    ],
    { cwd: root, timeout: 60000, maxBuffer: 1024 * 1024 },
  );
  assert.equal(native.stdout.trim(), "81920000000");
  const guest = summarize(await json(join(directory, "guest.json")));
  const report = {
    elapsedMs,
    timers,
    cpuSelfMs: [...self].sort((a, b) => b[1] - a[1]).slice(0, 15),
    guest: {
      samples: guest.samples,
      self: guest.self.slice(0, 12),
      inclusive: guest.inclusive.slice(0, 12),
    },
  };
  await save(join(directory, "profile.json"), report);
  console.log(JSON.stringify(report, null, 2));
}
const [command, first, second, rounds = "3"] = process.argv.slice(2);
if (command === "capture" && first) await capture(resolve(first));
else if (command === "compare" && first && second)
  await compare(resolve(first), resolve(second), Number(rounds));
else if (command === "profile" && first) await profile(resolve(first));
else
  throw Error(
    "usage: workflow-bench.mjs capture DIR | profile DIR | compare BEFORE AFTER [rounds=3]",
  );
