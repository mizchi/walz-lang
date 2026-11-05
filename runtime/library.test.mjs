import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
for (const backend of ["core", "component"])
  test(`${backend}: text library agrees with JSON and BigInt, preserving Unicode and every control byte`, async () => {
    const dir = await mkdtemp(join(tmpdir(), "gcwat-library-"));
    try {
      const { build } = await import(
        backend === "core" ? "./build-core.mjs" : "./build.mjs"
      );
      await build("tests/component-fixtures/text-library.gwat", dir);
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
        assert.equal(memory.live(), 0);
      }
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
