import { test } from "node:test";
import assert from "node:assert/strict";
import { createDelivery } from "./host.mjs";
import { Tasks } from "./tasks.mjs";
import { collect, tick } from "./testing/memory.mjs";
const gcOptions = { skip: typeof global.gc !== "function" };
test(
  "consumed and closed prefix streams release their payloads and tail",
  gcOptions,
  async () => {
    const create = async (close) => {
      const payload = new Uint8Array(1024 * 1024);
      const tail = {
        [Symbol.asyncIterator]() {
          return this;
        },
        next() {
          return { done: true };
        },
        return() {
          return { done: true };
        },
      };
      const refs = [new WeakRef(payload), new WeakRef(tail)];
      const stream = createDelivery([
        { id: 0, kind: "stream", unit: false },
      ]).prependT0([payload], tail);
      if (close) await stream.return();
      else {
        await stream.next();
        await stream.next();
      }
      return { stream, refs };
    };
    for (const close of [false, true]) {
      const { stream, refs } = await create(close);
      await collect();
      assert.ok(
        refs.every((ref) => ref.deref() === undefined),
        "closed streams must release payload and tail",
      );
      assert.deepEqual(await stream.next(), { done: true });
    }
  },
);
test(
  "a completed scalar task handle does not retain its runtime",
  gcOptions,
  async () => {
    const create = async (cancel) => {
      const tasks = new Tasks();
      tasks.flag = { value: 0 };
      tasks.raw = { value: () => 42 };
      const ref = new WeakRef(tasks);
      const task = tasks.start("value");
      if (cancel) await task.cancel();
      else assert.equal(await task.result, 42);
      await tick();
      return { task, ref };
    };
    for (const cancel of [false, true]) {
      const { task, ref } = await create(cancel);
      await collect();
      assert.ok(
        ref.deref() === undefined,
        `terminal ${cancel ? "cancelled" : "returned"} task must release its runtime`,
      );
      assert.equal(
        (await task.cancel()).status,
        cancel ? "cancelled" : "returned",
      );
    }
  },
);
