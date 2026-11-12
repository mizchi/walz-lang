import { test } from "node:test";
import assert from "node:assert/strict";
import { createHook } from "node:async_hooks";
import { Tasks } from "./tasks.mjs";

function instance(api, extra = {}) {
  const tasks = new Tasks(extra);
  const exports = { "cancel-requested": { value: 0 }, __task_cancel: new WebAssembly.Tag({ parameters: [] }),
    __task_cleanup: () => {}, "free-all": () => {} };
  tasks.capture({ exports });
  return { tasks, exports, ...tasks.bind({ api }) };
}
const tick = () => new Promise(resolve => setImmediate(resolve));

test("inline synchronous exports allocate no unobserved promises and preserve returned thenables", async () => {
  let thenCalls = 0;
  const endpoint = { then() { thenCalls++; } };
  const program = instance({ answer: () => 42, endpoint: () => endpoint });
  let promises = 0;
  const hook = createHook({ init(_id, type) { if (type === "PROMISE") promises++; } });
  hook.enable();
  let value;
  try { assert.equal(program.api.answer(), 42); value = program.api.endpoint(); }
  finally { hook.disable(); }
  assert.equal(value, endpoint);
  assert.equal(promises, 0, "inline calls have no asynchronous result observer");
  await tick(); assert.equal(thenCalls, 0, "a returned Future-like value must not be awaited internally");
  assert.equal(program.tasks.active, null);
});

test("start still publishes synchronous results, Future values, and completion acknowledgments", async () => {
  const endpoint = { then() { throw new Error("do not await endpoint"); }, resolveAsValue(resolve) {
    const then = this.then; this.then = undefined; try { resolve(this); } finally { this.then = then; }
  } };
  const program = instance({ answer: () => 42, endpoint: () => endpoint });
  const task = program.start("answer"); assert.equal(task.state, "queued");
  assert.equal(await task.result, 42); assert.deepEqual(await task.cancel(), { status: "returned", started: true });
  const future = program.start("endpoint"); assert.equal(await future.result, endpoint);
  assert.deepEqual(await future.cancel(), { status: "returned", started: true });
});

test("inline cleanup remains busy until disposal completes and then runs queued work", async () => {
  let release;
  const cleanup = new Promise(resolve => release = resolve);
  let program;
  program = instance({ value() { program.tasks.disposal(() => cleanup); return 42; }, answer: () => 7 });
  assert.equal(program.api.value(), 42);
  assert.throws(() => program.api.answer(), /busy/);
  const queued = program.start("answer");
  assert.equal(queued.state, "queued"); release();
  assert.equal(await queued.result, 7);
  assert.equal(program.tasks.active, null);
});

test("inline failure runs guest/boundary cleanup, resets flags, and preserves error identity", () => {
  const error = new Error("original");
  const program = instance({ fail() { throw error; } });
  let guest = 0, boundary = 0;
  program.tasks.guestCleanup = () => guest++; program.tasks.freeBoundary = () => boundary++;
  assert.throws(() => program.api.fail(), e => e === error);
  assert.equal(guest, 1); assert.equal(boundary, 1);
  assert.equal(program.exports["cancel-requested"].value, 0); assert.equal(program.tasks.active, null);
});
