import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { createTextIO, TEXT_IO, IOError } from "./text-io.mjs";
import { instrument, collect } from "./testing/memory.mjs";
import { workflowOracle } from "./workflow-oracle.mjs";
const encode = (s) => new TextEncoder().encode(s);
const exec = promisify(execFile);
const stress = process.env.WLT_WORKFLOW_STRESS === "1";

test(
  "keeping one workflow job does not retain unrelated jobs through the hash index",
  { skip: typeof global.gc !== "function" },
  async () => {
    const directory = await mkdtemp(join(tmpdir(), "wlt-job-reachability-"));
    try {
      const path = join(directory, "module.wasm");
      await exec("cargo", [
        "run",
        "--locked",
        "--quiet",
        "--",
        "build",
        "tests/component-fixtures/workflow-reachability.wlt",
        path,
      ]);
      const { instance } = await WebAssembly.instantiate(await readFile(path));
      const create = () => {
        const graph = instance.exports.new();
        return {
          kept: instance.exports.second(graph),
          other: new WeakRef(instance.exports.first(graph)),
        };
      };
      const { kept, other } = create();
      await collect();
      assert.equal(
        other.deref(),
        undefined,
        "a retained job must not root another job solely through lookup metadata",
      );
      assert.equal(instance.exports.duration(kept), 2n);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  },
);

test("workflow oracle has the specified simultaneous-completion and priority behavior", () => {
  const lines = [
    "TASK\ta\t3",
    "TASK\tb\t3",
    "TASK\tc\t1",
    "AFTER\tc\ta",
    "AFTER\tc\tb",
    "PLAN\t2",
  ];
  assert.deepEqual(workflowOracle(lines), [
    { line: 6, ok: true, kind: "plan", workers: 2, tasks: 3, makespan: 4 },
    { line: 6, kind: "job", id: "a", worker: 0, start: 0, finish: 3 },
    { line: 6, kind: "job", id: "b", worker: 1, start: 0, finish: 3 },
    { line: 6, kind: "job", id: "c", worker: 0, start: 3, finish: 4 },
  ]);
});

for (const backend of ["core", "component"])
  test(
    `${backend}: workflow plans match an independent event simulation and close I/O scopes`,
    { timeout: 180000 },
    async (t) => {
      const directory = await mkdtemp(join(tmpdir(), "wlt-workflow-"));
      try {
        const { build } = await import(
          backend === "core" ? "./build-core.mjs" : "./build.mjs"
        );
        await build("examples/apps/workflow/main.wlt", directory);
        await instrument(directory, backend);
        const runtime = await import(
          pathToFileURL(join(directory, "index.js"))
        );
        const open = async (input, write) => {
          let arena;
          const program = await runtime.instantiate({
            imports: { [TEXT_IO]: createTextIO({ input, write }) },
            instantiateCore: async (m, i) => {
              const instance = await WebAssembly.instantiate(m, i);
              if (instance.exports.alloc) arena = instance.exports;
              return instance;
            },
          });
          return {
            ...program,
            clean() {
              assert.equal(arena.live(), 0);
              assert.ok(
                Object.values(program.__memory()).every((n) => n === 0),
              );
            },
          };
        };
        const sample = (
          await readFile("examples/apps/workflow/sample.tsv", "utf8")
        )
          .trimEnd()
          .split("\n");
        const expectedSample = JSON.parse(
          await readFile("examples/apps/workflow/expected.json", "utf8"),
        );
        assert.deepEqual(workflowOracle(sample), expectedSample);
        const cli = await exec(process.execPath, [
          "runtime/text-cli.mjs",
          directory,
          "--input",
          "examples/apps/workflow/sample.tsv",
        ]);
        assert.equal(cli.stderr, "");
        assert.deepEqual(
          cli.stdout
            .trim()
            .split("\n")
            .map((line) => JSON.parse(line)),
          expectedSample,
        );
        const random = [];
        let seed = 9345;
        const next = () =>
          (seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0);
        for (let i = 0; i < 240; i++)
          random.push(`TASK	job${i}	${next() % 23}`);
        for (let i = 1; i < 240; i++)
          for (let n = 0, count = next() % 4; n < count; n++)
            random.push(`AFTER	job${i}	job${next() % i}`);
        for (const workers of [1, 2, 7, 32, 3]) random.push(`PLAN	${workers}`);
        const capacity = Array.from(
          { length: 4097 },
          (_, i) => `TASK	t${i}	0`,
        ).concat(["TASK\tt0\t1"]);
        const chain = Array.from(
          { length: 2200 },
          (_, i) => `TASK	c${i}	1000000`,
        );
        for (let i = 1; i < 2200; i++) chain.push(`AFTER	c${i}	c${i - 1}`);
        chain.push("PLAN\t7");
        const dense = Array.from({ length: 257 }, (_, i) => `TASK	d${i}	0`);
        for (let i = 0; i < 65536; i++)
          dense.push(`AFTER	d${i % 256}	d${i >>> 8}`);
        dense.push("AFTER\td256\td0", "AFTER\td0\td0");
        const cases = [
          sample,
          [],
          ["PLAN\t1", "PLAN\t32"],
          [
            "TASK\ta\t0",
            "TASK\tb\t0",
            "TASK\tc\t0",
            "AFTER\tb\ta",
            "AFTER\tc\tb",
            "PLAN\t1",
            "PLAN\t2",
          ],
          [
            "TASK\ta\t2",
            "TASK\tb\t3",
            "AFTER\ta\tb",
            "AFTER\tb\ta",
            "PLAN\t2",
            "TASK\tc\t1",
            "PLAN\t1",
          ],
          [
            "TASK\tself\t1",
            "AFTER\tself\tself",
            "AFTER\tself\tself",
            "PLAN\t3",
          ],
          [
            "TASK\t日本語🦀\t3",
            'TASK\tquote"\\\0\t2',
            'AFTER\tquote"\\\0\t日本語🦀',
            "PLAN\t2",
            "PLAN\t1",
          ],
          [
            "TASK\ta\t2",
            "TASK\ta\t9",
            "AFTER\ta\tmissing",
            "AFTER\tmissing\ta",
            "PLAN\t2",
          ],
          [
            "TASK\ta\t2",
            "PLAN\t2",
            "TASK\tb\t3",
            "AFTER\tb\ta",
            "PLAN\t2",
            "AFTER\ta\tb",
            "PLAN\t2",
          ],
          [
            "TASK\tdownstream\t1",
            "TASK\ta\t2",
            "TASK\tb\t3",
            "AFTER\tdownstream\ta",
            "AFTER\ta\tb",
            "AFTER\tb\ta",
            "PLAN\t2",
          ],
          // A newly unlocked earlier job precedes an already-ready later job.
          [
            "TASK\tc\t1",
            "TASK\ta\t1",
            "TASK\tb\t2",
            "TASK\td\t1",
            "AFTER\tc\ta",
            "PLAN\t2",
          ],
          // Hash collisions, resizing and references made before the resize.
          [
            "TASK\tAa\t1",
            "TASK\tBB\t2",
            "AFTER\tBB\tAa",
            ...Array.from({ length: 260 }, (_, i) => `TASK	key${i}	${i % 3}`),
            "TASK\tAa\t7",
            "AFTER\tkey259\tBB",
            "PLAN\t7",
            "PLAN\t1",
          ],
          [
            "",
            "# comment",
            "TASK\t\t1",
            "TASK\tx\t-1",
            "TASK\tx\t1000001",
            "PLAN\t0",
            "PLAN\t33",
            "PLAN\t2\textra",
            "TASK\tx\t1\textra",
            "AFTER\tx\t",
            "TASK\t" + "a".repeat(129) + "\t1",
            "TASK\t" + "🦀".repeat(32) + "\t000001",
            "PLAN\t2",
          ],
          random,
          capacity,
          chain,
          ...(stress ? [dense] : []),
        ];
        const timings = [];
        for (const lines of cases) {
          const records = [];
          let closed = 0;
          const input = (async function* () {
            try {
              for (const line of lines) {
                const data = encode(line + "\n");
                if (lines.length < 32) {
                  // Include boundaries inside multibyte UTF-8 identifiers.
                  for (let i = 0; i < data.length; i += 3)
                    yield data.subarray(i, i + 3);
                } else {
                  const cut = Math.min(2, data.length);
                  yield data.subarray(0, cut);
                  yield data.subarray(cut);
                }
              }
            } finally {
              closed++;
            }
          })();
          const program = await open(input, async (bytes) => {
            records.push(JSON.parse(new TextDecoder().decode(bytes)));
          });
          const started = performance.now();
          assert.deepEqual(await program.api.main(), {
            tag: "ok",
            val: lines.length,
          });
          const elapsedMs = performance.now() - started;
          assert.deepEqual(
            records,
            workflowOracle(lines),
            `backend ${backend}; ${lines.length} lines`,
          );
          assert.equal(closed, 1);
          program.clean();
          timings.push({
            lines: lines.length,
            records: records.length,
            elapsedMs,
          });
          if (lines.length > 1000)
            t.diagnostic(
              `${backend}: ${lines.length} input lines, ${records.length} output records: ${elapsedMs.toFixed(1)} ms`,
            );
        }
        for (const phase of ["read", "write"]) {
          let closed = 0,
            writes = 0;
          const program = await open(
            (async function* () {
              try {
                yield phase === "read"
                  ? new Uint8Array([255, 10])
                  : encode("PLAN\t1\n");
              } finally {
                closed++;
              }
            })(),
            async () => {
              writes++;
              throw Error("disk full");
            },
          );
          assert.deepEqual(await program.api.main(), {
            tag: "err",
            val: phase === "read" ? IOError.utf8 : IOError.write,
          });
          assert.equal(writes, phase === "read" ? 0 : 1);
          assert.equal(closed, 1);
          program.clean();
        }
        for (const phase of ["read", "write"]) {
          const entered = Promise.withResolvers(),
            disposed = Promise.withResolvers(),
            release = Promise.withResolvers();
          let closed = 0;
          const input =
            phase === "read"
              ? {
                  [Symbol.asyncIterator]() {
                    return this;
                  },
                  next() {
                    entered.resolve();
                    return new Promise(() => {});
                  },
                  async return() {
                    closed++;
                    disposed.resolve();
                    await release.promise;
                    return { done: true };
                  },
                }
              : (async function* () {
                  try {
                    yield encode(
                      "TASK\ta\t3\nTASK\tb\t2\nAFTER\tb\ta\nPLAN\t2\n",
                    );
                  } finally {
                    closed++;
                  }
                })();
          const program = await open(input, async (_, signal) => {
            entered.resolve();
            await new Promise((r) =>
              signal.addEventListener("abort", r, { once: true }),
            );
            disposed.resolve();
            await release.promise;
          });
          const task = program.start("main"),
            rejected = assert.rejects(task.result, {
              name: "TaskCancelledError",
            });
          await entered.promise;
          let acknowledged = false;
          const cancellation = task.cancel().then((r) => {
            acknowledged = true;
            return r;
          });
          await disposed.promise;
          assert.equal(acknowledged, false);
          release.resolve();
          assert.equal((await cancellation).status, "cancelled");
          await rejected;
          assert.equal(closed, 1);
          program.clean();
        }
        // Reuse one instance with a fresh explicit I/O capability for each call.
        const repeated = ["TASK\ta\t1", "TASK\tb\t2", "AFTER\tb\ta", "PLAN\t2"];
        let io,
          output,
          reusedArena,
          closed = 0;
        const program = await runtime.instantiate({
          imports: {
            [TEXT_IO]: {
              readLines() {
                io = createTextIO({
                  input: (async function* () {
                    try {
                      yield encode(repeated.join("\n") + "\n");
                    } finally {
                      closed++;
                    }
                  })(),
                  write: async (bytes) =>
                    output.push(JSON.parse(new TextDecoder().decode(bytes))),
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
            if (instance.exports.alloc) reusedArena = instance.exports;
            return instance;
          },
        });
        for (let round = 0; round < 20; round++) {
          output = [];
          assert.deepEqual(await program.api.main(), { tag: "ok", val: 4 });
          assert.deepEqual(output, workflowOracle(repeated));
          assert.equal(reusedArena.live(), 0);
          assert.ok(Object.values(program.__memory()).every((n) => n === 0));
        }
        assert.equal(closed, 20);
        // The same graph operations without per-line I/O help distinguish
        // guest algorithm costs from the text/stream/ABI path above.
        const computeDirectory = join(directory, "compute");
        await build("examples/apps/workflow/benchmark.wlt", computeDirectory);
        const computeRuntime = await import(
          pathToFileURL(join(computeDirectory, "index.js"))
        );
        const compute = await computeRuntime.instantiate();
        let started = performance.now();
        assert.equal(await compute.api.chain(2200, 1), 2200000000n);
        const pureChainMs = performance.now() - started;
        started = performance.now();
        assert.equal(await compute.api.edgeLimit(), 1);
        const pureDenseMs = performance.now() - started;
        t.diagnostic(
          `${backend}: without I/O: chain ${pureChainMs.toFixed(1)} ms, dense edge limit ${pureDenseMs.toFixed(1)} ms`,
        );
        await mkdir("target/workflow-validation", { recursive: true });
        await writeFile(
          `target/workflow-validation/${backend}-${stress ? "stress" : "regular"}.json`,
          JSON.stringify(
            {
              backend,
              node: process.version,
              stress,
              cases: timings,
              pureChainMs,
              pureDenseMs,
              reuseRounds: 20,
              inputAndWriteCancellation: true,
              ioFailures: true,
              liveHandlesAfterEachCall: 0,
            },
            null,
            2,
          ) + "\n",
        );
      } finally {
        await rm(directory, { recursive: true, force: true });
      }
    },
  );
