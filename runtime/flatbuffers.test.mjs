import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { Builder, ByteBuffer } from "flatbuffers";
import * as flatbuffers from "flatbuffers";
import { instrument } from "./testing/memory.mjs";
import { createReference } from "./flatbuffers-bench/reference.mjs";
import { verify } from "./flatbuffers-bench/runner.mjs";

// Independent schema adapter using the official implementation, rather than
// comparing byte layouts (FlatBuffers permits multiple layouts for one value).
function officialEncode(message, { unknown = false, force = false } = {}) {
  const b = new Builder(16);
  b.forceDefaults(force);
  const name = b.createString(message.name);
  b.startVector(4, message.values.length, 4);
  for (let i = message.values.length - 1; i >= 0; i--) b.addInt32(message.values[i]);
  const values = b.endVector();
  const payload = b.createByteVector(message.payload);
  let child = 0;
  if (message.child) {
    const label = b.createString(message.child.label);
    b.startObject(2);
    b.addFieldInt32(0, message.child.count, 0);
    b.addFieldOffset(1, label, 0);
    b.endObject(); // Unreferenced twin exercises official vtable deduplication.
    b.startObject(2);
    b.addFieldInt32(0, message.child.count, 0);
    b.addFieldOffset(1, label, 0);
    child = b.endObject();
  }
  b.startObject(unknown ? 8 : 7);
  b.addFieldInt64(0, message.id, 0n);
  b.addFieldOffset(1, name, 0);
  b.addFieldOffset(2, values, 0);
  b.addFieldOffset(3, payload, 0);
  b.addFieldInt8(4, Number(message.enabled), 1);
  b.addFieldFloat64(5, message.weight, 0);
  if (child) b.addFieldOffset(6, child, 0);
  if (unknown) b.addFieldInt32(7, 42, 0);
  b.finish(b.endObject(), "WLZF");
  return b.asUint8Array().slice();
}

function field(bb, table, slot) { return bb.__offset(table, 4 + slot * 2); }
function pointer(bb, table, slot) {
  const off = field(bb, table, slot);
  return off ? table + off : 0;
}
function text(bb, table, slot) {
  const p = pointer(bb, table, slot);
  return p ? bb.__string(p) : "";
}
function officialDecode(bytes) {
  const bb = new ByteBuffer(bytes);
  assert.equal(bb.getBufferIdentifier(), "WLZF");
  const table = bb.readUint32(0);
  const read = (slot, fn, fallback) => {
    const p = pointer(bb, table, slot);
    return p ? fn.call(bb, p) : fallback;
  };
  const vector = (slot, width, fn) => {
    const p = pointer(bb, table, slot);
    if (!p) return [];
    return Array.from({ length: bb.__vector_len(p) }, (_, i) => fn.call(bb, bb.__vector(p) + i * width));
  };
  const p = pointer(bb, table, 6), child = p ? bb.__indirect(p) : 0;
  return {
    id: read(0, bb.readInt64, 0n), name: text(bb, table, 1),
    values: Int32Array.from(vector(2, 4, bb.readInt32)), payload: Uint8Array.from(vector(3, 1, bb.readUint8)),
    enabled: Boolean(read(4, bb.readUint8, 1)), weight: read(5, bb.readFloat64, 0),
    child: child ? {
      count: pointer(bb, child, 0) ? bb.readInt32(pointer(bb, child, 0)) : 0,
      label: text(bb, child, 1),
    } : undefined,
  };
}

const empty = () => ({ id: 0n, name: "", values: new Int32Array(), payload: new Uint8Array(), enabled: true, weight: 0, child: undefined });
const reference = createReference(flatbuffers);
const sample = () => ({
  id: -9223372036854775808n, name: "日本語🦀\0hello",
  values: Int32Array.of(-2147483648, -1, 0, 2147483647), payload: Uint8Array.of(0, 1, 128, 255),
  enabled: false, weight: -1.25, child: { count: 42, label: "child🦀" },
});

for (const backend of ["core", "component"]) {
  test(`${backend}: FlatBuffers interoperates, validates hostile input and releases ABI handles`, { timeout: 120000 }, async () => {
    const capture = process.env.WALZ_FLATBUFFERS_CAPTURE;
    if (capture) await verify(resolve(capture));
    const directory = capture ? join(resolve(capture), backend) : await mkdtemp(join(tmpdir(), "walz-flatbuffers-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      if (!capture) {
        await build("examples/flatbuffers/main.walz", directory);
        await instrument(directory, backend);
      }
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      let arena;
      const { api, __memory } = await instantiate({ instantiateCore: async (module, imports) => {
        const instance = await WebAssembly.instantiate(module, imports);
        if (instance.exports.live) arena = instance.exports;
        return instance;
      } });
      const clean = () => {
        assert.equal(arena.live(), 0, "linear ABI buffers are released");
        assert.ok(Object.values(__memory()).every(n => n === 0), "host handle tables are empty");
      };
      const decode = bytes => {
        // Portable Result returns use the runtime's explicit tagged-value policy.
        const result = api.decode(bytes);
        clean();
        return result;
      };
      const bad = (bytes, label, code) => {
        const result = decode(bytes);
        assert.equal(result.tag, "err", label);
        if (code !== undefined) assert.equal(result.val, code, label);
      };
      assert.equal(api.main(), 42);
      for (const value of [empty(), sample(), { ...sample(), id: 9223372036854775807n, name: "x".repeat(8192), values: Int32Array.from({ length: 1025 }, (_, i) => i - 512) }]) {
        const encoded = api.encode(value);
        clean();
        assert.deepEqual(officialDecode(encoded), value, "Walz → official JavaScript");
        assert.deepEqual(reference.decode(encoded), value, "benchmark reference reads Walz output");
        assert.deepEqual(decode(reference.encode(value)), { tag: "ok", val: value }, "Walz reads benchmark reference output");
        assert.deepEqual(decode(encoded), { tag: "ok", val: value }, "Walz roundtrip");
        for (const options of [{}, { unknown: true }, { force: true }])
          assert.deepEqual(decode(officialEncode(value, options)), { tag: "ok", val: value }, `official JavaScript → Walz ${JSON.stringify(options)}`);
      }
      for (const weight of [Infinity, -Infinity, NaN]) {
        const value = { ...empty(), weight };
        assert.deepEqual(officialDecode(api.encode(value)), value);
        assert.deepEqual(decode(officialEncode(value)), { tag: "ok", val: value });
      }
      for (const n of [0, 1, 2, 3, 4, 5, 7, 8, 15, 16, 17, 63, 64, 65, 255, 256, 257, 65537]) {
        const value = { ...empty(), name: "x".repeat(n % 17),
          values: Int32Array.from({ length: n % 258 }, (_, i) => Math.imul(i - 129, 65537)),
          payload: Uint8Array.from({ length: n }, (_, i) => (i * 31 + 7) & 255) };
        assert.deepEqual(officialDecode(api.encode(value)), value, `copy/alignment boundary ${n}`);
        assert.deepEqual(decode(officialEncode(value)), { tag: "ok", val: value });
      }
      // Preserve IEEE signed zero when Walz emits the field explicitly.
      assert.ok(Object.is(officialDecode(api.encode({ ...empty(), weight: -0 })).weight, -0));
      // A valid vtable can be after its table: the signed distance is negative.
      const negative = Uint8Array.of(8, 0, 0, 0, 87, 76, 90, 70, 252, 255, 255, 255, 4, 0, 4, 0);
      assert.deepEqual(decode(negative), { tag: "ok", val: empty() });
      const valid = officialEncode(sample());
      const mutate32 = (at, value) => {
        const copy = valid.slice(); new DataView(copy.buffer).setUint32(at, value, true); return copy;
      };
      const bb = new ByteBuffer(valid), root = bb.readUint32(0), vt = root - bb.readInt32(root);
      const child = bb.__indirect(pointer(bb, root, 6));
      assert.ok(bb.readInt32(child) < 0, "official builder shared a vtable after the child table");
      const string = bb.__indirect(pointer(bb, root, 1));
      const vector = bb.__indirect(pointer(bb, root, 2));
      bad(valid.slice(0, 7), "short identifier/root", 1);
      bad(mutate32(0, 0xffffffff), "root offset overflow");
      bad(mutate32(0, root + 1), "misaligned root", 2);
      bad(mutate32(4, 0), "wrong file identifier", 3);
      bad(mutate32(root, 0x80000000), "signed vtable distance overflow");
      bad(mutate32(root, 0), "table cannot be its own vtable");
      const smallVtable = valid.slice(); new DataView(smallVtable.buffer).setUint16(vt, 3, true);
      bad(smallVtable, "invalid vtable size");
      const badField = valid.slice(); new DataView(badField.buffer).setUint16(vt + 4, 1, true);
      bad(badField, "field overlaps vtable offset word");
      bad(mutate32(string, 0xffffffff), "string length overflow");
      bad(mutate32(vector, 0x40000000), "vector byte length overflow");
      bad(mutate32(pointer(bb, root, 6), 0xfffffff0), "nested table offset overflow");
      const utf8 = valid.slice(); utf8[string + 4] = 0xff; bad(utf8, "invalid UTF-8", 4);
      const nul = valid.slice(); nul[string + 4 + bb.readUint32(string)] = 1; bad(nul, "missing string terminator");
      // Every prefix and deterministic random bytes must return Result, never trap.
      for (let end = 0; end < valid.length; end++) decode(valid.slice(0, end));
      let seed = 0x12345678;
      const next = () => (seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0);
      for (let i = 0; i < 300; i++) {
        const input = i % 2 ? valid.slice() : Uint8Array.from({ length: next() % 192 }, () => next() >>> 24);
        if (i % 2) input[next() % input.length] = next() >>> 24;
        decode(input);
      }
      for (let i = 0; i < 100; i++) {
        assert.deepEqual(decode(api.encode(sample())), { tag: "ok", val: sample() });
        bad(new Uint8Array(), "repeated failures release ABI handles too");
      }
      clean();
    } finally { if (!capture) await rm(directory, { recursive: true, force: true }); }
  });
}
