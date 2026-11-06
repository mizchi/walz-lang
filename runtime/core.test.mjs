import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtemp,
  rm,
  writeFile,
  readFile,
  readdir,
  stat,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "./build-core.mjs";
import { collect, tick } from "./testing/memory.mjs";

async function fixture(source, run) {
  const directory = await mkdtemp(join(tmpdir(), "wlt-core-"));
  try {
    const input = join(directory, "input.wlt");
    await writeFile(input, source);
    await build(input, directory);
    await run(
      await import(pathToFileURL(join(directory, "index.js"))),
      directory,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

const source = `(module
  (func $main (export "main") (param $x i64) (result i64) (%effect pure) (local.get $x))
  (func $transfer (export "transfer") (param $x (%tuple (%future i64) (%tuple %string %unit)))
    (result (%tuple (%future i64) (%tuple %string %unit))) (%effect pure) (%move $x))
  (func $option (export "option") (param $x (%option (%option i64)))
    (result (%option (%option i64))) (%effect pure) (local.get $x)))`;

test("Core bindings roll back partially encoded owned inputs and preserve nested option distinctions", async () => {
  await fixture(source, async (runtime) => {
    let allocator,
      closed = 0;
    const { api } = await runtime.instantiate({
      instantiateCore: async (m, i) => {
        const instance = await WebAssembly.instantiate(m, i);
        if (instance.exports.alloc) allocator = instance.exports;
        return instance;
      },
    });
    const input = Object.assign(Promise.resolve(42n), {
      [Symbol.asyncDispose]() {
        closed++;
      },
    });
    assert.throws(
      () => api.transfer([input, ["allocated 🦀", "invalid"]]),
      /unit/,
    );
    await tick();
    assert.equal(closed, 1);
    assert.equal(allocator.live(), 0);
    assert.equal(api.main(42n), 42n);
    const pair = api.transfer([Promise.resolve(42n), ["日本語", "value"]]);
    assert.equal(await pair[0], 42n);
    assert.deepEqual(pair[1], ["日本語", "value"]);
    for (const option of [
      { tag: "none" },
      { tag: "some", val: undefined },
      { tag: "some", val: 42n },
    ])
      assert.deepEqual(api.option(option), option);
    assert.equal(allocator.live(), 0);
  });
});

test("scalar Core output has no Component dependency or text codecs", async () => {
  await fixture(
    "(module (func $f (export \"main\") (result i64) (%effect pure) (i64.const 42)))",
    async (runtime, directory) => {
      assert.equal((await runtime.instantiate()).api.main(), 42n);
      const names = await readdir(directory);
      assert.ok(
        !names.includes("component.js") && !names.includes("component.wasm"),
      );
      const binding = await readFile(join(directory, "bindings.mjs"), "utf8");
      assert.ok(
        !binding.includes("TextEncoder") && !binding.includes("Float64"),
      );
      // Count the entire executable JS graph, including the scheduler and host.
      let bytes = 0;
      for (const name of names.filter((n) => /\.(?:m?js)$/.test(n)))
        bytes += (await stat(join(directory, name))).size;
      assert.ok(bytes < 32768, `Core JS graph is ${bytes} bytes`);
    },
  );
});

test(
  "exported ends and cancelled late producers do not retain Core instances",
  { skip: typeof global.gc !== "function" },
  async () => {
    const directory = await mkdtemp(join(tmpdir(), "wlt-core-gc-"));
    try {
      await build("tests/component-fixtures/values.wlt", directory);
      const runtime = await import(pathToFileURL(join(directory, "index.js")));
      const create = async (cancel) => {
        let memory, notify, complete;
        const ready = new Promise((r) => (notify = r));
        const pending = new Promise((r) => (complete = r));
        const { api, start } = await runtime.instantiate({
          onTaskEvent: (e) => {
            if (e.type === "read-pending") notify();
          },
          instantiateCore: async (m, i) => {
            const instance = await WebAssembly.instantiate(m, i);
            if (instance.exports.alloc) memory = instance.exports.memory;
            return instance;
          },
        });
        const future = api.future(),
          stream = api.stream();
        if (cancel) {
          const task = start("await", [pending]);
          const rejected = assert.rejects(
            task.result,
            (e) => e.name === "TaskCancelledError",
          );
          await ready;
          await task.cancel();
          await rejected;
        }
        return { ref: new WeakRef(memory), future, stream, pending, complete };
      };
      for (const cancel of [false, true]) {
        const held = await create(cancel);
        await collect();
        assert.equal(
          held.ref.deref(),
          undefined,
          "external ends and a detached read must release the Wasm store",
        );
        assert.equal(await held.future, 42n);
        assert.deepEqual(await Array.fromAsync(held.stream), [40n, 2n]);
        held.complete(17n);
        await tick();
      }
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  },
);

test("Core keeps the existing shared-payload restriction for delivery", async () => {
  await assert.rejects(
    fixture(
      `(module
    (func $nested (export "nested") (result (%future (%future i64))) (%effect pure)
      (%future.ready (%future i64) (%future.ready i64 (i64.const 42)))))`,
      () => {},
    ),
    /delivery payload must be shared/,
  );
});

test(
  "failed task history does not retain Core stores through lazy error stacks",
  { skip: typeof global.gc !== "function" },
  async () => {
    await fixture(
      `(module
    (func $value (export "value") (param $n i64) (result i64) (%effect pure) (local.get $n))
    (func $crash (export "crash") (%effect pure) (unreachable))
    (func $await (export "await") (param $f (%future i64)) (result i64) (%effect suspends)
      (%future.await i64 (%move $f))))`,
      async (runtime) => {
        const create = async (name, args) => {
          let memory, notify;
          const ready = new Promise((resolve) => {
            notify = resolve;
          });
          const { start } = await runtime.instantiate({
            onTaskEvent: (event) => {
              if (event.type === "read-pending") notify();
            },
            instantiateCore: async (m, i) => {
              const instance = await WebAssembly.instantiate(m, i);
              if (instance.exports.alloc) memory = instance.exports.memory;
              return instance;
            },
          });
          const task = start(name, args);
          const rejected = task.result.catch((error) => error);
          if (name === "await") {
            await ready;
            await task.cancel();
          }
          const error = await rejected;
          assert.ok(error instanceof Error);
          await tick();
          return { task, error, ref: new WeakRef(memory) };
        };
        for (const [name, args] of [
          ["value", ["wrong"]],
          ["crash", []],
          [
            "await",
            [
              Object.assign(new Promise(() => {}), {
                [Symbol.asyncDispose]() {
                  throw new Error("cleanup failed");
                },
              }),
            ],
          ],
        ]) {
          const history = await create(name, args);
          await collect();
          assert.equal(
            history.ref.deref(),
            undefined,
            "keeping the failed handle and error must release its instance",
          );
          assert.equal(history.task.state, "failed");
        }
      },
    );
  },
);

test("host failures stay fatal through catch_all and falsy rejection reasons", async () => {
  await fixture(
    `(module
    (import "wlt:host/event-loop@0.1.0" "sleep" (func $sleep (param $ms i32)))
    (func $direct (export "direct") (%effect host suspends) (call $sleep (i32.const 0)))
    (func $caught (export "caught") (result i64) (%effect host suspends)
      (%try (result i64) (do (block (result i64) (call $sleep (i32.const 0)) (i64.const 42)))
        (catch_all (i64.const 99)))))`,
    async (runtime) => {
      for (const reason of [new Error("host failed"), 0, undefined]) {
        for (const name of ["direct", "caught"]) {
          const { api } = await runtime.instantiate({
            eventLoop: { sleep: () => Promise.reject(reason) },
          });
          await assert.rejects(api[name](), (error) =>
            reason instanceof Error
              ? error === reason
              : error instanceof WebAssembly.RuntimeError &&
                error.cause === reason,
          );
          await assert.rejects(api.direct(), /cannot enter component instance/);
        }
      }
    },
  );
});
