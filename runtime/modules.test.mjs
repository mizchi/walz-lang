import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

for (const backend of ["core", "component"])
  test(`${backend}: a selected file export runs without the library's unused clock capability`, async () => {
    const directory = await mkdtemp(join(tmpdir(), "wlt-module-imports-"));
    try {
      const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
      await build("tests/component-fixtures/module-imports.wlt", directory);
      const manifest = JSON.parse(await readFile(join(directory,
        backend === "core" ? "manifest.json" : "component.wasm.json")));
      assert.deepEqual(manifest.imports, []);
      await instrument(directory, backend);
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      const program = await instantiate(), { api } = program;
      assert.equal(api.answer(), 42);
      assert.deepEqual(Object.keys(api), ["answer"]);
      assert.ok(Object.values(program.__memory()).every((n) => n === 0));
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
