import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

for (const backend of ["core", "component"])
  test(`${backend}: wlt:core types and grouped bindings preserve async portable ABIs`, async () => {
    const directory = await mkdtemp(join(tmpdir(), "wlt-standard-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/standard.wlt", directory);
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      const calls = [];
      let memory;
      const { api } = await instantiate({
        eventLoop: { deferI64: async (ms, value) => { calls.push(ms); return value; } },
        instantiateCore: async (module, imports) => {
          const instance = await WebAssembly.instantiate(module, imports);
          if (instance.exports.live) memory = instance.exports;
          return instance;
        },
      });
      assert.deepEqual(await api.main(42n), { tag: "ok", val: "42" });
      assert.deepEqual(calls, [0]);
      assert.equal(api.optional(0), undefined);
      assert.equal(api.optional(1), "present");
      assert.deepEqual(await api.future(), { tag: "ok", val: "ready" });
      assert.equal(memory.live(), 0);
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
