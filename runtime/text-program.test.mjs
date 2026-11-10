import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { createTextIO, TEXT_IO, IOError } from "./text-io.mjs";
import { instrument } from "./testing/memory.mjs";
const encode = (text) => new TextEncoder().encode(text);
const barrier = () => Promise.withResolvers();

for (const backend of ["core", "component"])
  test(
    `${backend}: real text capabilities preserve values, errors, backpressure and cancellation`,
    { timeout: 60000 },
    async () => {
      const dir = await mkdtemp(join(tmpdir(), "walz-io-"));
      try {
        const { build } = await import(
          backend === "core" ? "./build-core.mjs" : "./build.mjs"
        );
        await build("examples/io/text-filter.walz", dir);
        await instrument(dir, backend);
        const runtime = await import(pathToFileURL(join(dir, "index.js")));
        await assert.rejects(runtime.instantiate(), /missing host capability/);
        const instantiate = async (io) => {
          let allocator;
          const program = await runtime.instantiate({
            imports: { [TEXT_IO]: io },
            instantiateCore: async (m, i) => {
              const instance = await WebAssembly.instantiate(m, i);
              if (instance.exports.alloc) allocator = instance.exports;
              return instance;
            },
          });
          return {
            ...program,
            live: () => {
              assert.ok(
                Object.values(program.__memory()).every((n) => n === 0),
              );
              return allocator.live();
            },
          };
        };
        const bytes = encode("日本語🦀\r\n\nlast");
        const output = [];
        const io = createTextIO({
          input: (async function* () {
            for (const byte of bytes) yield new Uint8Array([byte]);
          })(),
          write: async (data) => output.push(new TextDecoder().decode(data)),
        });
        const success = await instantiate(io);
        assert.deepEqual(await success.api.main(), { tag: "ok", val: 2 });
        assert.deepEqual(output, ["日本語🦀\n", "last\n"]);
        assert.equal(success.live(), 0);
        for (const [data, sink, code] of [
          [new Uint8Array([0xff, 10]), async () => {}, IOError.utf8],
          [
            encode("line\n"),
            async () => {
              throw new Error("no space");
            },
            IOError.write,
          ],
        ]) {
          let closed = 0;
          const program = await instantiate(
            createTextIO({
              input: (async function* () {
                try {
                  yield data;
                } finally {
                  closed++;
                }
              })(),
              write: sink,
            }),
          );
          assert.deepEqual(await program.api.main(), { tag: "err", val: code });
          assert.equal(closed, 1);
          assert.equal(program.live(), 0);
        }
        for (const phase of ["read", "write"]) {
          const entered = barrier(),
            release = barrier(),
            late = barrier(),
            disposing = barrier();
          let closed = 0,
            aborted = false;
          const input =
            phase === "read"
              ? {
                  [Symbol.asyncIterator]() {
                    return this;
                  },
                  next() {
                    entered.resolve();
                    return late.promise;
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
                    yield encode("pending\n");
                  } finally {
                    closed++;
                  }
                })();
          const write =
            phase === "read"
              ? async () => {
                  assert.fail("no output expected");
                }
              : async (data, signal) => {
                  entered.resolve();
                  await new Promise((resolve) =>
                    signal.addEventListener(
                      "abort",
                      () => {
                        aborted = true;
                        disposing.resolve();
                        resolve();
                      },
                      { once: true },
                    ),
                  );
                  await release.promise;
                };
          const program = await instantiate(createTextIO({ input, write }));
          const task = program.start("main");
          const rejected = assert.rejects(
            task.result,
            (error) => error.name === "TaskCancelledError",
          );
          await entered.promise;
          let acknowledged = false;
          const cancelling = task.cancel().then((result) => {
            acknowledged = true;
            return result;
          });
          await disposing.promise;
          assert.equal(acknowledged, false);
          if (phase === "write") assert.equal(aborted, true);
          release.resolve();
          assert.deepEqual(await cancelling, {
            status: "cancelled",
            started: true,
          });
          await rejected;
          assert.equal(closed, 1);
          assert.equal(program.live(), 0);
          late.resolve({ done: false, value: encode("late\n") });
          await new Promise((resolve) => setImmediate(resolve));
          assert.equal(program.live(), 0);
        }
      } finally {
        await rm(dir, { recursive: true, force: true });
      }
    },
  );

for (const backend of ["core", "component"])
  test(`${backend}: flattened text imports handle empty and Unicode values without retaining buffers`, async () => {
    const directory = await mkdtemp(join(tmpdir(), "walz-flat-text-"));
    try {
      const { writeFile } = await import("node:fs/promises");
      const source = join(directory, "source.walz");
      await writeFile(
        source,
        `(module
      (import "walz:host/text-io@0.1.0" "write-line" (func $write (param $string @string) (result (@future (@result @unit i32)))))
      (func $main (export "main") (param $string @string) (result (@result @unit i32)) (@effect host suspends)
        (@future.await (@result @unit i32) (call $write (local.get $string)))))`,
      );
      const { build } = await import(
        backend === "core" ? "./build-core.mjs" : "./build.mjs"
      );
      await build(source, directory);
      const { instantiate } = await import(
        pathToFileURL(join(directory, "index.js"))
      );
      let allocator;
      const output = [];
      const { api } = await instantiate({
        imports: {
          [TEXT_IO]: createTextIO({
            write: async (bytes) =>
              output.push(
                new TextDecoder("utf-8", { ignoreBOM: true }).decode(bytes),
              ),
          }),
        },
        instantiateCore: async (m, i) => {
          const instance = await WebAssembly.instantiate(m, i);
          if (instance.exports.alloc) allocator = instance.exports;
          return instance;
        },
      });
      for (const value of ["", "日本語🦀", "\0", "\ufeff"]) {
        const result = await api.main(value);
        assert.equal(result.tag, "ok");
        assert.equal(result.val, undefined);
        assert.equal(output.at(-1), value + "\n");
        assert.equal(allocator.live(), 0);
      }
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
