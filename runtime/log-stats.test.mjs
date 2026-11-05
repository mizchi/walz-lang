import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import { TEXT_IO, createTextIO, IOError } from "./text-io.mjs";
import { instrument } from "./testing/memory.mjs";
const exec = promisify(rawExec);
const source = "examples/apps/log-stats/main.gwat";
const encode = (s) => new TextEncoder().encode(s);

for (const backend of ["core", "component"])
  test(
    `${backend}: streaming log statistics run in the guest, close I/O and retain no ABI buffers`,
    { timeout: 120000 },
    async () => {
      const dir = await mkdtemp(join(tmpdir(), "gcwat-log-stats-"));
      try {
        const { build } = await import(
          backend === "core" ? "./build-core.mjs" : "./build.mjs"
        );
        await build(source, dir);
        await instrument(dir, backend);
        const runtime = await import(pathToFileURL(join(dir, "index.js")));
        const open = async (input, write, extra = {}) => {
          let memory;
          const program = await runtime.instantiate({
            ...extra,
            imports: { [TEXT_IO]: createTextIO({ input, write }) },
            instantiateCore: async (m, i) => {
              const instance = await WebAssembly.instantiate(m, i);
              if (instance.exports.alloc) memory = instance.exports;
              return instance;
            },
          });
          return {
            ...program,
            clean() {
              assert.equal(memory.live(), 0);
              assert.equal(memory.memory.buffer.byteLength, 65536);
              assert.ok(
                Object.values(program.__memory()).every((n) => n === 0),
              );
            },
          };
        };
        const sample = await readFile("examples/apps/log-stats/sample.tsv");
        const expected = JSON.parse(
          await readFile("examples/apps/log-stats/expected.json", "utf8"),
        );
        const output = [];
        let closed = 0;
        const program = await open(
          (async function* () {
            try {
              for (const byte of sample) yield new Uint8Array([byte]);
            } finally {
              closed++;
            }
          })(),
          async (bytes) => output.push(Buffer.from(bytes)),
        );
        assert.deepEqual(await program.api.main(), { tag: "ok", val: 0 });
        assert.deepEqual(
          JSON.parse(Buffer.concat(output).toString()),
          expected,
        );
        assert.equal(closed, 1);
        program.clean();
        // A large input uses fixed-size aggregation state and multiple I/O chunks.
        const large = [];
        const bulk = await open(
          (async function* () {
            for (let i = 0; i < 200; i++)
              yield encode(
                "INFO\t7\t日本語🦀\nWARN\t2\twork\nERROR\t9\tfail\nbad\n\n".repeat(
                  25,
                ),
              );
          })(),
          async (bytes) => large.push(Buffer.from(bytes)),
        );
        assert.deepEqual(await bulk.api.main(), { tag: "ok", val: 0 });
        assert.deepEqual(JSON.parse(Buffer.concat(large).toString()), {
          accepted: 15000,
          rejected: 5000,
          ignored: 5000,
          levels: {
            INFO: { count: 5000, total_ms: 35000, max_ms: 7 },
            WARN: { count: 5000, total_ms: 10000, max_ms: 2 },
            ERROR: { count: 5000, total_ms: 45000, max_ms: 9 },
          },
        });
        bulk.clean();
        const empty = [];
        const noInput = await open((async function* () {})(), async (bytes) =>
          empty.push(Buffer.from(bytes)),
        );
        assert.deepEqual(await noInput.api.main(), { tag: "ok", val: 0 });
        assert.equal(JSON.parse(Buffer.concat(empty).toString()).accepted, 0);
        noInput.clean();
        // Input failure stops before reporting; output failure is an explicit Result.
        for (const phase of ["read", "write"]) {
          let returned = 0,
            writes = 0;
          const bad = await open(
            (async function* () {
              try {
                yield phase === "read"
                  ? new Uint8Array([255, 10])
                  : encode("INFO\t1\tok\n");
              } finally {
                returned++;
              }
            })(),
            async () => {
              writes++;
              throw new Error("disk full");
            },
          );
          assert.deepEqual(await bad.api.main(), {
            tag: "err",
            val: phase === "read" ? IOError.utf8 : IOError.write,
          });
          assert.equal(returned, 1);
          assert.equal(writes, phase === "read" ? 0 : 1);
          bad.clean();
        }
        // Both input and report writes can suspend. Cancellation waits for cleanup.
        for (const phase of ["read", "write"]) {
          const entered = Promise.withResolvers(),
            releasing = Promise.withResolvers(),
            disposing = Promise.withResolvers();
          let disposed = 0;
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
                    disposed++;
                    disposing.resolve();
                    await releasing.promise;
                    return { done: true };
                  },
                }
              : (async function* () {
                  try {
                    yield encode("INFO\t1\tok\n");
                  } finally {
                    disposed++;
                  }
                })();
          const waiting = await open(input, async (_, signal) => {
            entered.resolve();
            await new Promise((resolve) =>
              signal.addEventListener("abort", resolve, { once: true }),
            );
            disposing.resolve();
            await releasing.promise;
          });
          const task = waiting.start("main");
          const rejected = assert.rejects(task.result, {
            name: "TaskCancelledError",
          });
          await entered.promise;
          let acknowledged = false;
          const cancel = task.cancel().then((result) => {
            acknowledged = true;
            return result;
          });
          await disposing.promise;
          assert.equal(acknowledged, false);
          releasing.resolve();
          assert.equal((await cancel).status, "cancelled");
          await rejected;
          assert.equal(disposed, 1);
          waiting.clean();
        }
        if (backend === "core") {
          const cli = await exec(process.execPath, [
            "runtime/text-cli.mjs",
            dir,
            "--input",
            "examples/apps/log-stats/sample.tsv",
          ]);
          assert.deepEqual(JSON.parse(cli.stdout), expected);
        }
      } finally {
        await rm(dir, { recursive: true, force: true });
      }
    },
  );
