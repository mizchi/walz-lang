import { test } from "node:test";
import assert from "node:assert/strict";
import { createTextIO, IOError } from "./text-io.mjs";

const encode = (text) => new TextEncoder().encode(text);
async function* chunks(values) {
  yield* values;
}

test("UTF-8 line input handles split codepoints, CRLF, blank lines and a final unterminated line", async () => {
  const bytes = encode("\ufeff日本語🦀\r\n\nend");
  const io = createTextIO({
    input: chunks(Array.from(bytes, (byte) => new Uint8Array([byte]))),
  });
  assert.deepEqual(await Array.fromAsync(io.readLines()), [
    { tag: "ok", val: "\ufeff日本語🦀" },
    { tag: "ok", val: "" },
    { tag: "ok", val: "end" },
  ]);
});

test("invalid UTF-8, bounded lines and read failures become one Result error then EOF", async () => {
  for (const [input, limit, code] of [
    [chunks([new Uint8Array([0x61, 0xff, 0x0a])]), 16, IOError.utf8],
    [chunks([encode("abcd")]), 3, IOError.limit],
    [
      (async function* () {
        throw new Error("read failed");
      })(),
      16,
      IOError.read,
    ],
  ]) {
    const io = createTextIO({ input, maxLineBytes: limit });
    assert.deepEqual(await Array.fromAsync(io.readLines()), [
      { tag: "err", val: code },
    ]);
  }
});

test("closing a pending read interrupts its provider and waits for its cleanup", async () => {
  let reading,
    release,
    closed = 0;
  const ready = new Promise((r) => (reading = r)),
    barrier = new Promise((r) => (release = r));
  const input = {
    [Symbol.asyncIterator]() {
      return this;
    },
    next() {
      reading();
      return new Promise(() => {});
    },
    async return() {
      closed++;
      await barrier;
      return { done: true };
    },
  };
  const io = createTextIO({ input });
  const lines = io.readLines();
  const pending = lines.next();
  await ready;
  let done = false;
  const closing = lines.return().then(() => (done = true));
  await Promise.resolve();
  assert.equal(done, false);
  assert.deepEqual(await pending, { done: true });
  release();
  await closing;
  assert.equal(closed, 1);
  await lines.return();
  assert.equal(closed, 1);
});

test("output waits for the sink and cancellation acknowledges its cleanup", async () => {
  let finish,
    started,
    closed = 0;
  const ready = new Promise((r) => (started = r));
  const io = createTextIO({
    write: (bytes, signal) => {
      assert.deepEqual(bytes, encode("日本語\n"));
      started();
      return new Promise((resolve) => {
        finish = resolve;
        signal.addEventListener(
          "abort",
          () => {
            closed++;
            resolve();
          },
          { once: true },
        );
      });
    },
  });
  const operation = io.writeLine("日本語");
  await ready;
  await operation[Symbol.asyncDispose]();
  assert.equal(closed, 1);
  assert.deepEqual(await operation, { tag: "err", val: IOError.closed });
  finish();
  const bad = createTextIO({
    write: async () => {
      throw new Error("full");
    },
  });
  assert.deepEqual(await bad.writeLine("x"), {
    tag: "err",
    val: IOError.write,
  });
});

test(
  "closed line readers release their provider, chunks and return values",
  { skip: typeof global.gc !== "function" },
  async () => {
    const { collect } = await import("./testing/memory.mjs");
    const create = async () => {
      const bytes = encode("line\n");
      const provider = {
        [Symbol.asyncIterator]() {
          return this;
        },
        async next() {
          return { done: false, value: bytes };
        },
        async return() {
          return { done: true, value: this };
        },
      };
      const refs = [new WeakRef(bytes), new WeakRef(provider)];
      const io = createTextIO({ input: provider });
      const reader = io.readLines();
      await reader.next();
      await reader.return();
      return { io, reader, refs };
    };
    const held = await create();
    await collect();
    assert.ok(held.refs.every((ref) => ref.deref() === undefined));
    assert.deepEqual(await held.reader.next(), { done: true });
  },
);
