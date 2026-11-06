import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
const { build } = await import(
  process.env.WLT_BACKEND === "core" ? "./build-core.mjs" : "./build.mjs"
);
import { createEventLoop } from "./host.mjs";

async function fixture(name, fn) {
  const dir = await mkdtemp(join(tmpdir(), "wlt-component-"));
  try {
    await build(`tests/component-fixtures/${name}.wlt`, dir);
    const runtime = await import(pathToFileURL(join(dir, "index.js")));
    await fn(runtime, dir);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

test(
  "Canonical ABI roundtrips signed bits, variants, units and readable endpoints",
  { timeout: 60000 },
  async () => {
    await fixture("values", async (runtime) => {
      let memory;
      const { api, start } = await runtime.instantiate({
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.alloc) memory = instance.exports.memory;
          return instance;
        },
      });
      assert.equal(api.value(undefined), undefined);
      assert.deepEqual(api.value({ tag: "ok", val: -9223372036854775808n }), {
        tag: "ok",
        val: -9223372036854775808n,
      });
      assert.deepEqual(api.value({ tag: "err", val: -1.25 }), {
        tag: "err",
        val: -1.25,
      });
      assert.deepEqual(api.units(["value", undefined]), ["value", undefined]);
      assert.deepEqual(api.units(["value", "value"]), ["value", "value"]);
      assert.equal(await api.future(), 42n);
      assert.equal(
        await api.await(new Promise((r) => setTimeout(() => r(42n), 5))),
        42n,
      );
      const items = [];
      for await (const n of api.stream()) items.push(n);
      assert.deepEqual(items, [40n, 2n]);
      const prepended = [];
      for await (const n of api.prepend(
        (async function* () {
          await new Promise((r) => setTimeout(r, 1));
          yield 2n;
        })(),
      ))
        prepended.push(n);
      assert.deepEqual(prepended, [40n, 2n]);
      assert.equal(await api.first([42n]), 42n);
      assert.equal(await api.first([]), undefined);
      let closed = 0;
      const tail = {
        [Symbol.asyncIterator]() {
          return this;
        },
        next() {
          throw new Error("close must not read the source");
        },
        async return() {
          closed++;
          return { done: true };
        },
      };
      const discarded = api.prepend(tail)[Symbol.asyncIterator]();
      await discarded.return();
      await new Promise((resolve) => setTimeout(resolve, 0));
      assert.equal(
        closed,
        1,
        "close must propagate through an unread prefix to its remote tail",
      );
      await start("closeStream", [tail]).result;
      assert.equal(closed, 2);
      assert.equal(await api.unitFuture(), undefined);
      assert.equal(
        await api.awaitUnit(new Promise((r) => setTimeout(() => r(), 1))),
        undefined,
      );
      const units = [];
      for await (const unit of api.unitStream()) units.push(unit);
      assert.deepEqual(units, [undefined]);
      assert.equal(
        await api.nextUnit(
          (async function* () {
            await new Promise((r) => setTimeout(r, 1));
            yield;
          })(),
        ),
        "value",
      );
      assert.equal(await api.nextUnit((async function* () {})()), undefined);
      const options = [];
      for await (const n of api.optionStream()) options.push(n);
      assert.deepEqual(options, [undefined, 42n]);
      assert.deepEqual(
        api.wide(...Array.from({ length: 17 }, (_, i) => (i === 0 ? 26 : i))),
        [42, -1.25],
      );
      const bytes = memory.buffer.byteLength;
      for (let i = 0; i < 20000; i++) {
        assert.deepEqual(api.value({ tag: "ok", val: BigInt(i) }), {
          tag: "ok",
          val: BigInt(i),
        });
        assert.deepEqual(api.wide(...Array(17).fill(i)), [i * 2, -1.25]);
      }
      assert.equal(
        memory.buffer.byteLength,
        bytes,
        "post-return and indirect parameter buffers must be reused",
      );
      assert.throws(() => api.units(["wrong", undefined]));
      const { api: failing } = await runtime.instantiate();
      await assert.rejects(
        failing.await(Promise.reject(new Error("producer failed"))),
        /promise failed/,
      );
    });
  },
);

test(
  "JSPI yields to the event loop, retains GC roots, queues calls and drops pending futures",
  { timeout: 60000 },
  async () => {
    await fixture("event-loop", async (runtime) => {
      const events = [];
      const host = createEventLoop({ onEvent: (event) => events.push(event) });
      const { api } = await runtime.instantiate({ eventLoop: host });
      const order = [];
      const slow = api.main(30).then((n) => {
        order.push("slow");
        return n;
      });
      const fast = api.main(0).then((n) => {
        order.push("fast");
        return n;
      });
      const tick = new Promise((r) =>
        setTimeout(() => {
          order.push("tick");
          r();
        }, 0),
      );
      assert.deepEqual(
        await Promise.all([slow, fast, tick]).then((v) => v.slice(0, 2)),
        [42n, 42n],
      );
      assert.ok(order.indexOf("tick") < order.indexOf("slow"));
      // Synchronous canonical entry points queue calls to this instance while a
      // guest stack is suspended. The surrounding JavaScript loop stays live.
      assert.ok(order.indexOf("slow") < order.indexOf("fast"));
      assert.equal(await api.sumTicks(5, 1), 10n);
      assert.equal(await api.sumTicks(0, 0), 0n);
      assert.equal(await api.completionError(17), 17);
      assert.equal(await api.completionError(0), 0);
      await api.closePending(60000);
      assert.ok(events.some((e) => e.type === "dispose"));
      await assert.rejects(api.main(-1));
      await assert.rejects(api.main(0), /cannot enter component instance/);
    });
  },
);

test(
  "bytes/text copy through the Canonical ABI, validate UTF-8 and reclaim nested buffers",
  { timeout: 60000 },
  async () => {
    await fixture("buffers", async (runtime, dir) => {
      let memory, live;
      const { api } = await runtime.instantiate({
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.alloc) {
            memory = instance.exports.memory;
            live = instance.exports.live;
          }
          return instance;
        },
      });
      for (const text of [
        "",
        "ascii\0\n",
        "日本語🦀",
        "e\u0301",
        "\u{10ffff}",
      ]) {
        assert.equal(api.text(text), text);
        const bytes = new TextEncoder().encode(text);
        assert.deepEqual(api.encode(text), bytes);
        assert.deepEqual(api.bytes(bytes), bytes);
        assert.equal(api.decode(bytes), text);
        assert.equal(api.length(text), bytes.length);
        assert.deepEqual(api.nested([text, bytes]), [text, bytes]);
        assert.deepEqual(api.variant({ tag: "ok", val: text }), {
          tag: "ok",
          val: text,
        });
        assert.deepEqual(api.variant({ tag: "err", val: bytes }), {
          tag: "err",
          val: bytes,
        });
        assert.equal(api.wide(...Array(9).fill(text)), text);
        assert.equal(live(), 0);
      }
      assert.equal(api.literal(), "日本語🦀");
      assert.deepEqual(api.newBytes(), new Uint8Array([0, 255, 0, 255]));
      const input = new Uint8Array([0, 255, 128, 42]);
      const output = api.bytes(input);
      input.fill(99);
      assert.deepEqual(output, new Uint8Array([0, 255, 128, 42]));
      assert.deepEqual(api.slice(output, 1, 2), new Uint8Array([255, 128]));
      assert.deepEqual(api.slice(output, 4, 0), new Uint8Array());
      for (let start = 0; start <= output.length; start++)
        for (let length = 0; length <= output.length - start; length++)
          assert.deepEqual(api.slice(output, start, length), output.slice(start, start + length));
      assert.equal(api.get(output, 1), 255);
      assert.equal(await api.ready("🦀"), "🦀");
      assert.equal(
        await api.await(new Promise((r) => setTimeout(() => r("日本語"), 1))),
        "日本語",
      );
      const items = [];
      for await (const item of api.stream(output)) items.push(item);
      assert.deepEqual(items, [output]);
      assert.deepEqual(
        await api.first(
          (async function* () {
            yield output;
          })(),
        ),
        output,
      );
      assert.equal(await api.first([]), undefined);
      assert.deepEqual(await api.first([new Uint8Array()]), new Uint8Array());
      assert.equal(live(), 0);
      // Compare the actual Wasm decoder with an independent UTF-8 oracle.
      const decoder = new TextDecoder("utf-8", {
        fatal: true,
        ignoreBOM: true,
      });
      const check = (bytes) => {
        let expected;
        try {
          expected = decoder.decode(bytes);
        } catch {
          expected = undefined;
        }
        assert.equal(api.decode(bytes), expected, `UTF-8 ${Array.from(bytes)}`);
      };
      // Byte sequences placed at every position around the ASCII fast path,
      // including incomplete, overlong, surrogate and out-of-range sequences.
      for (let offset = 0; offset <= 24; offset++) {
        const prefix = Array(offset).fill(97);
        for (const payload of [
          [0],
          [0x80],
          [0xc0, 0x80],
          [0xed, 0xa0, 0x80],
          [0xf4, 0x90, 0x80, 0x80],
          [0xf0, 0x9f],
          Array.from(new TextEncoder().encode("日本語🦀abc")),
        ]) {
          check(new Uint8Array([...prefix, ...payload]));
          check(new Uint8Array([...prefix, ...payload, ...Array(16).fill(98)]));
        }
      }
      for (let a = 0; a < 256; a++) {
        check(new Uint8Array([a]));
        for (let b = 0; b < 256; b++) check(new Uint8Array([a, b]));
      }
      for (const bytes of [
        [0xe0, 0xa0, 0x80],
        [0xed, 0x9f, 0xbf],
        [0xed, 0xa0, 0x80],
        [0xf0, 0x90, 0x80, 0x80],
        [0xf4, 0x8f, 0xbf, 0xbf],
        [0xf4, 0x90, 0x80, 0x80],
        [0xef, 0xbb, 0xbf],
        [0xc0, 0x80],
        [0xe2, 0x82],
        [0x61, 0x80],
      ])
        check(new Uint8Array(bytes));
      let seed = 42;
      for (let n = 0; n < 3000; n++) {
        const bytes = Uint8Array.from({ length: n % 9 }, () => {
          seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
          return seed >>> 24;
        });
        check(bytes);
      }
      const pages = memory.buffer.byteLength;
      for (let i = 0; i < 2000; i++) {
        assert.deepEqual(api.nested(["x".repeat(100), output]), [
          "x".repeat(100),
          output,
        ]);
        assert.equal(live(), 0);
      }
      assert.equal(memory.buffer.byteLength, pages);
      const types = await import("node:fs/promises").then((fs) =>
        fs.readFile(
          join(
            dir,
            process.env.WLT_BACKEND === "core"
              ? "bindings.mjs"
              : "interfaces/wlt-runtime-delivery.d.ts",
          ),
          "utf8",
        ),
      );
      assert.match(types, /Uint8Array/);
      // Cancel reads of dynamically allocated payloads before allowing a late
      // producer to resolve. No detached read may allocate into the freed arena.
      for (const name of ["await", "first"]) {
        let notifyRead, finish, countLive;
        const reading = new Promise((r) => {
          notifyRead = r;
        });
        const pending = new Promise((r) => {
          finish = r;
        });
        let closed = 0;
        const input =
          name === "await"
            ? Object.assign(pending, {
                [Symbol.asyncDispose]() {
                  closed++;
                },
              })
            : {
                [Symbol.asyncIterator]() {
                  return this;
                },
                next() {
                  return pending;
                },
                return() {
                  closed++;
                  return { done: true };
                },
              };
        const { api: other, start } = await runtime.instantiate({
          onTaskEvent(e) {
            if (e.type === "read-pending") notifyRead();
          },
          instantiateCore: async (module, imports) => {
            const instance = await WebAssembly.instantiate(module, imports);
            if (instance.exports.live) countLive = instance.exports.live;
            return instance;
          },
        });
        const task = start(name, [input]);
        const rejected = assert.rejects(
          task.result,
          (e) => e.name === "TaskCancelledError",
        );
        await reading;
        assert.equal((await task.cancel()).status, "cancelled");
        await rejected;
        assert.equal(closed, 1);
        assert.equal(countLive(), 0);
        finish(
          name === "await" ? "遅い配送🦀" : { done: false, value: output },
        );
        await new Promise((r) => setTimeout(r, 0));
        assert.equal(countLive(), 0);
        assert.equal(other.text("alive"), "alive");
      }
      // Traps lock the component, so use an independent instance per probe.
      for (const invoke of [
        (api) => api.get(output, -1),
        (api) => api.slice(output, 3, 2),
        (api) => api.slice(output, -1, 0),
        (api) => api.slice(output, 1, 0x7fffffff),
        (api) => api.slice(output, 0, -1),
        (api) => api.slice(output, output.length + 1, 0),
      ]) {
        const { api: bad } = await runtime.instantiate();
        assert.throws(() => invoke(bad));
      }
    });
  },
);
