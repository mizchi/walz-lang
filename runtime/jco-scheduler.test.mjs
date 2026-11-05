import { test } from "node:test";
import assert from "node:assert/strict";
import vm from "node:vm";
import { tuneScheduler } from "./jco-scheduler.mjs";

const loop = `"use components";
async function turn(result){
const delay = result === ComponentAsyncState.TickResult.RESUMED ? 0 : 10;
await new Promise((resolve) => setTimeout(resolve, delay));
}
`;

test("resumptions yield a real task turn after microtasks without a timer delay; idle polling keeps its delay", async () => {
  const events = [],
    context = vm.createContext({
      Promise,
      ComponentAsyncState: { TickResult: { RESUMED: 1 } },
      setImmediate(fn) {
        events.push("task queued");
        setImmediate(fn);
      },
      setTimeout(fn, delay) {
        events.push(`timer ${delay}`);
        setImmediate(fn);
      },
    });
  vm.runInContext(tuneScheduler(loop), context);
  const running = vm
    .runInContext("turn(1)", context)
    .then(() => events.push("resumed"));
  Promise.resolve().then(() => events.push("microtask"));
  assert.deepEqual(events, ["task queued"]);
  await running;
  assert.deepEqual(events, ["task queued", "microtask", "resumed"]);
  await vm.runInContext("turn(0)", context);
  assert.equal(events.at(-1), "timer 10");
});

test("browser MessageChannel turns coalesce queued callbacks and schedule another turn on reentry", async () => {
  const messages = [],
    contexts = [];
  class MessageChannel {
    constructor() {
      contexts.push(this);
      this.port1 = {};
      this.port2 = {
        postMessage: () => messages.push(() => this.port1.onmessage()),
      };
    }
  }
  const context = vm.createContext({
    Promise,
    MessageChannel,
    ComponentAsyncState: { TickResult: { RESUMED: 1 } },
    setTimeout() {
      throw Error("unexpected polling timer");
    },
  });
  vm.runInContext(tuneScheduler(loop), context);
  const first = vm.runInContext("turn(1)", context),
    second = vm.runInContext("turn(1)", context);
  assert.equal(messages.length, 1);
  messages.shift()();
  await Promise.all([first, second]);
  const third = vm.runInContext("turn(1)", context);
  assert.equal(contexts.length, 1);
  assert.equal(messages.length, 1);
  messages.shift()();
  await third;
});

test("pinned loop drift fails closed; generated modules without that scheduler are untouched", () => {
  assert.equal(
    tuneScheduler("export function instantiate(){}"),
    "export function instantiate(){}",
  );
  assert.throws(
    () => tuneScheduler("class ComponentAsyncState { async runTickLoop() {} }"),
    /scheduler patch no longer matches/,
  );
});

test("platforms without task-posting APIs retain the timer fallback", async () => {
  const delays = [];
  const context = vm.createContext({
    Promise,
    ComponentAsyncState: { TickResult: { RESUMED: 1 } },
    setTimeout(resolve, delay) {
      delays.push(delay);
      resolve();
    },
  });
  vm.runInContext(tuneScheduler(loop), context);
  await vm.runInContext("turn(1)", context);
  assert.deepEqual(delays, [0]);
});
