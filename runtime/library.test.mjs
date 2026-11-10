import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
for (const backend of ["core", "component"])
  test(`${backend}: text library agrees with JSON and BigInt, preserving Unicode and every control byte`, async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-library-"));
    try {
      const { build } = await import(
        backend === "core" ? "./build-core.mjs" : "./build.mjs"
      );
      await build("tests/component-fixtures/text-library.walz", dir);
      const { instantiate } = await import(
        pathToFileURL(join(dir, "index.js"))
      );
      let memory;
      const { api } = await instantiate({
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.alloc) memory = instance.exports;
          return instance;
        },
      });
      for (const text of [
        "",
        "日本語🦀\ufeff",
        '"\\',
        Array.from({ length: 32 }, (_, i) => String.fromCharCode(i)).join(""),
        "a".repeat(4096),
      ]) {
        assert.equal(JSON.parse(api.quote(text)), text);
        assert.equal(api.join(text, "🦀"), text + "🦀");
        assert.equal(memory.live(), 0);
      }
      const encode = (s) => new TextEncoder().encode(s);
      for (const [text, needle] of [["aaab", "aab"], ["🦀::日本語::", "::"], ["", ""], ["x", "x"], ["::", "::"], ["abc", "z"], ["a\0b", "\0"]]) {
        const bytes = encode(text), pattern = encode(needle);
        const found = Buffer.from(bytes).indexOf(Buffer.from(pattern));
        assert.equal(api.find(bytes, pattern, 0), found < 0 ? undefined : found);
        const i = text.indexOf(needle);
        assert.deepEqual(api.split(text, needle), i < 0 ? undefined : [text.slice(0, i), text.slice(i + needle.length)]);
        assert.equal(api.find(bytes, pattern, -1), undefined);
      }
      assert.equal(api.findText("🦀", "", 1), undefined);
      assert.equal(api.findText("🦀", "", 4), 4);
      assert.equal(api.findByte(encode("a\t🦀\t"), 2, 9), 6);
      assert.equal(api.findByte(encode("a"), 0, 256), 1);
      assert.equal(api.findByte(encode("a"), -1, 97), 1);
      for (let seed = 0; seed < 32; seed++) {
        const bytes = Uint8Array.from({ length: seed }, (_, i) => (i * 73 + seed * 11) & 255);
        for (const pattern of [bytes.slice(2, 5), new Uint8Array([0, 255]), new Uint8Array()])
          for (const start of [0, 3, bytes.length, bytes.length + 1]) {
            const found = start <= bytes.length ? Buffer.from(bytes).indexOf(Buffer.from(pattern), start) : -1;
            assert.equal(api.find(bytes, pattern, start), found < 0 ? undefined : found);
          }
      }
      for (const text of ["", "-", "+1", " 1", "1 ", "1.0", "１", "1\0", "18446744073709551616", "-9223372036854775809", "9223372036854775808", "-0", "00042"]) {
        const unsigned = /^[0-9]+$/.test(text) && BigInt(text) <= 2n ** 64n - 1n;
        const signed = /^-?[0-9]+$/.test(text) && BigInt(text) >= -(2n ** 63n) && BigInt(text) < 2n ** 63n;
        assert.equal(api.parseUnsigned(text), unsigned ? BigInt.asIntN(64, BigInt(text)) : undefined, text);
        assert.equal(api.parseSigned(text), signed ? BigInt(text) : undefined, text);
      }
      assert.equal(api.parseRange(encode("x42!"), 1, 2, 42n), 42n);
      assert.equal(api.parseRange(encode("x42!"), 1, 2, 41n), undefined);
      assert.equal(api.parseRange(encode("0"), 0, 1, 0n), 0n);
      for (const [start, length] of [[2, 1], [0, 0], [-1, 1], [0, -1]])
        assert.equal(api.parseRange(encode("1"), start, length, -1n), undefined);
      assert.equal(api.repeat("日本語🦀", 2048), "日本語🦀".repeat(2048));
      assert.equal(api.repeat("", 2048), "");
      const document = api.document('key"\\\n', '日本語🦀\0', 42n);
      assert.deepEqual(JSON.parse(document), { ['key"\\\n']: '日本語🦀\0', unsigned: 42, signed: 42, ok: true });
      assert.equal(memory.live(), 0);
      let value = 42n;
      for (const n of [
        0n,
        -1n,
        -9223372036854775808n,
        9223372036854775807n,
        ...Array.from({ length: 32 }, () => {
          value = BigInt.asIntN(64, value * 6364136223846793005n + 1n);
          return value;
        }),
      ]) {
        assert.equal(api.unsigned(n), BigInt.asUintN(64, n).toString());
        assert.equal(api.signed(n), n.toString());
        assert.equal(api.parseUnsigned(BigInt.asUintN(64, n).toString()), n);
        assert.equal(api.parseSigned(n.toString()), n);
        assert.equal(memory.live(), 0);
      }
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
