import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import { createTextIO, TEXT_IO, IOError } from "./text-io.mjs";
import { instrument } from "./testing/memory.mjs";
const exec = promisify(rawExec),
  encode = (s) => new TextEncoder().encode(s);
const directory = "examples/apps/stock-ledger";
const oracle = (lines) => {
  const stocks = new Map(),
    output = [];
  for (const line of lines) {
    if (!line || line.startsWith("#")) continue;
    const [op, key, quantity, ...extra] = line.split("\t");
    if (
      !key ||
      Buffer.byteLength(key) > 256 ||
      extra.length ||
      !["ADD", "TAKE", "SHOW"].includes(op) ||
      (op === "SHOW"
        ? quantity !== undefined
        : quantity === undefined ||
          !/^\d+$/.test(quantity) ||
          BigInt(quantity) > 4294967295n)
    ) {
      output.push({ ok: false, error: "invalid-command" });
      continue;
    }
    const old = stocks.get(key) ?? 0n,
      n = BigInt(quantity ?? 0);
    if (op === "TAKE" && n > old) {
      output.push({ op, key, ok: false, error: "insufficient-stock" });
      continue;
    }
    const value = op === "ADD" ? old + n : op === "TAKE" ? old - n : old;
    if (op === "ADD" || (op === "TAKE" && stocks.has(key)))
      stocks.set(key, value);
    output.push({ op, key, ok: true, quantity: Number(value) });
  }
  return output;
};
for (const backend of ["core", "component"])
  test(
    `${backend}: inventory operations agree with a Map oracle, scoped cleanup and the explicit control-flow version`,
    { timeout: 180000 },
    async () => {
      const root = await mkdtemp(join(tmpdir(), "walz-stock-"));
      try {
        const { build } = await import(
          backend === "core" ? "./build-core.mjs" : "./build.mjs"
        );
        const runtimes = [];
        for (const name of ["main", "main-explicit", "main-counted"]) {
          const output = join(root, name);
          await build(`${directory}/${name}.walz`, output);
          await instrument(output, backend);
          runtimes.push({
            ...(await import(pathToFileURL(join(output, "index.js")))),
            countsLines: name === "main-counted",
          });
        }
        const open = async (runtime, input, write) => {
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
              assert.equal(arena.memory.buffer.byteLength, 65536);
              assert.ok(
                Object.values(program.__memory()).every((n) => n === 0),
              );
            },
          };
        };
        const sample = await readFile(`${directory}/sample.tsv`, "utf8");
        assert.deepEqual(
          oracle(sample.trimEnd().split("\n")),
          JSON.parse(await readFile(`${directory}/expected.json`, "utf8")),
        );
        const cases = [
          sample.trimEnd().split("\n"),
          [],
          [
            'ADD\ta"b\\🦀\t0',
            'ADD\ta"b\\🦀\t4294967295',
            'TAKE\ta"b\\🦀\t1',
            'SHOW\ta"b\\🦀',
            "ADD\t\t1",
            "SHOW\tx\t1",
            "ADD\tx\t-1",
            "ADD\tx\t4294967296",
            "ADD\tx\t1\textra",
            "ADD\tleading\t00042",
            "TAKE\tleading\t0001",
            "SHOW\tx\t",
            "ADD\tx\t+1",
            "ADD\tx\t 1",
            "ADD\t" + "🦀".repeat(64) + "\t1",
            "ADD\t" + "a".repeat(257) + "\t1",
          ],
        ];
        let seed = 42;
        const random = [];
        for (let i = 0; i < 3000; i++) {
          seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
          const op = ["ADD", "TAKE", "SHOW"][seed % 3],
            key = [
              "日本語🦀",
              "Aa",
              "BB",
              '"quoted"',
              "back\\slash",
              "nul\0key",
              "key" + (seed % 97),
            ][(seed >>> 8) % 7];
          random.push(
            op + "\t" + key + (op === "SHOW" ? "" : "\t" + (seed % 20)),
          );
        }
        cases.push(random);
        for (const lines of cases)
          for (const runtime of runtimes) {
            const output = [];
            let closed = 0;
            const data = encode(lines.join("\n") + (lines.length ? "\n" : ""));
            const program = await open(
              runtime,
              (async function* () {
                try {
                  for (let i = 0; i < data.length; i += 17)
                    yield data.subarray(i, i + 17);
                } finally {
                  closed++;
                }
              })(),
              async (bytes) =>
                output.push(JSON.parse(new TextDecoder().decode(bytes))),
            );
            assert.deepEqual(await program.api.main(), {
              tag: "ok",
              val: runtime.countsLines ? lines.length : 0,
            });
            assert.deepEqual(output, oracle(lines));
            assert.equal(closed, 1);
            program.clean();
          }
        // Business failures are report records; I/O failures terminate the stream.
        for (const runtime of runtimes)
          for (const phase of ["read", "write"]) {
            let closed = 0,
              writes = 0;
            const program = await open(
              runtime,
              (async function* () {
                try {
                  yield phase === "read"
                    ? new Uint8Array([255, 10])
                    : encode("ADD\ta\t1\nSHOW\ta\n");
                } finally {
                  closed++;
                }
              })(),
              async () => {
                writes++;
                throw new Error("full");
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
        for (const runtime of runtimes)
          for (const phase of ["read", "write"]) {
            const entered = Promise.withResolvers(),
              disposing = Promise.withResolvers(),
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
                      disposing.resolve();
                      await release.promise;
                      return { done: true };
                    },
                  }
                : (async function* () {
                    try {
                      yield encode("ADD\ta\t1\n");
                    } finally {
                      closed++;
                    }
                  })();
            const program = await open(runtime, input, async (_, signal) => {
              entered.resolve();
              await new Promise((r) =>
                signal.addEventListener("abort", r, { once: true }),
              );
              disposing.resolve();
              await release.promise;
            });
            const task = program.start("main"),
              rejected = assert.rejects(task.result, {
                name: "TaskCancelledError",
              });
            await entered.promise;
            let acknowledged = false;
            const cancelled = task.cancel().then((r) => {
              acknowledged = true;
              return r;
            });
            await disposing.promise;
            assert.equal(acknowledged, false);
            release.resolve();
            assert.equal((await cancelled).status, "cancelled");
            await rejected;
            assert.equal(closed, 1);
            program.clean();
          }
        const errors = join(root, "errors");
        await build(
          "tests/component-fixtures/stock-ledger-errors.walz",
          errors,
        );
        await instrument(errors, backend);
        const errorsRuntime = await import(
          pathToFileURL(join(errors, "index.js"))
        );
        const records = [];
        const errorsProgram = await open(
          errorsRuntime,
          (async function* () {})(),
          async (bytes) =>
            records.push(JSON.parse(new TextDecoder().decode(bytes))),
        );
        assert.deepEqual(await errorsProgram.api.main(), { tag: "ok", val: 0 });
        assert.deepEqual(records, [
          { op: "ADD", key: "max", ok: false, error: "quantity-overflow" },
          { op: "ADD", key: "new", ok: false, error: "capacity" },
        ]);
        errorsProgram.clean();
        if (backend === "core") {
          const result = await exec(process.execPath, [
            "runtime/text-cli.mjs",
            join(root, "main"),
            "--input",
            `${directory}/sample.tsv`,
          ]);
          assert.deepEqual(
            result.stdout.trim().split("\n").map(JSON.parse),
            oracle(sample.trimEnd().split("\n")),
          );
        }
      } finally {
        await rm(root, { recursive: true, force: true });
      }
    },
  );
