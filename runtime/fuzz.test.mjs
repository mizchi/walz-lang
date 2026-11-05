import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { randomUUID } from "node:crypto";
import { createEventLoop } from "./host.mjs";
import { instrument } from "./testing/memory.mjs";

for (const backend of ["core", "component"])
  test(`${backend}: seeded cancellation schedules preserve outcomes and release all scopes`, { timeout: 60000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "wlt-fuzz-cancel-"));
    const seed = Number(process.env.WLT_FUZZ_SEED ?? 42) >>> 0;
    let state = seed;
    const random = () => ((state = (Math.imul(state, 1664525) + 1013904223) >>> 0) >>> 8);
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/event-loop.wlt", directory);
      await instrument(directory, backend);
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      let entered, control, calls, disposed;
      const runtime = await instantiate({ eventLoop: {
        ...createEventLoop(),
        sleep() {
          calls++;
          control = Promise.withResolvers();
          let closed = false;
          control.promise[Symbol.asyncDispose] = () => {
            if (closed) throw new Error("duplicate disposal");
            closed = true; disposed++; control.resolve();
          };
          entered.resolve();
          return control.promise;
        },
      } });
      const seen = new Set();
      for (let index = 0; index < 32; index++) {
        const mode = index < 4 ? index : random() % 4, steps = random() % 5;
        seen.add(mode);
        try {
          entered = Promise.withResolvers(); calls = disposed = 0;
          const task = runtime.start("catchAll", [1]);
          const result = task.result.then(value => ({ value }), error => ({ error }));
          const reason = `seed-${seed}-${index}`;
          if (mode !== 0) await entered.promise;
          if (mode >= 2) {
            control.resolve();
            for (let i = 0; i < steps; i++) await Promise.resolve();
          }
          if (mode === 3) assert.deepEqual(await result, { value: 42n });
          const ack = await task.cancel(reason), outcome = await result;
          if (outcome.error) {
            assert.equal(outcome.error.name, "TaskCancelledError");
            assert.equal(outcome.error.cause, reason);
            assert.equal(ack.status, "cancelled");
          } else {
            assert.equal(outcome.value, 42n);
            assert.equal(ack.status, "returned");
          }
          if (mode === 0) { assert.equal(calls, 0); assert.equal(ack.started, false); }
          if (mode === 1) { assert.equal(disposed, 1); assert.equal(ack.status, "cancelled"); }
          assert.ok(disposed <= 1);
          assert.deepEqual(await task.cancel("second"), ack);
          assert.ok(Object.values(runtime.__memory()).every(n => n === 0));
        } catch (error) {
          await mkdir("target/fuzz/failures", { recursive: true });
          const file = `target/fuzz/failures/cancel-${backend}-${randomUUID()}.json`;
          await writeFile(file, JSON.stringify({ seed, index, mode, steps, backend, error: error.stack }, null, 2), { flag: "wx" });
          throw new Error(`cancellation seed ${seed}, case ${index}; ${file}`, { cause: error });
        }
      }
      assert.equal(seen.size, 4);
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
