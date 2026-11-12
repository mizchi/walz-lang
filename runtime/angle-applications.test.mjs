import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";

for (const backend of ["core", "component", "native-utf8", "native-js-strings"])
  test(`${backend}: angle applications preserve generic bodies and nested value ABIs`, { timeout: 120000 }, async () => {
    const directory = await mkdtemp(join(tmpdir(), "walz-angle-applications-"));
    try {
      if (backend.startsWith("native-")) {
        const { build } = await import("./build-js-native.mjs");
        await build("examples/angle-applications.walz", directory, { mode: backend.slice(7) });
      } else {
        const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
        await build("examples/angle-applications.walz", directory);
        await instrument(directory, backend);
      }
      const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
      const value = await instantiate();
      assert.equal(value.api.main(), 42);
      for (const n of [-2147483648, -1, 0, 1, 40, 2147483647]) {
        assert.deepEqual(value.api.split(n), n < 0 ? { tag: "err", val: 1 } : { tag: "ok", val: [n, 2] });
        const memory = value.__memory();
        if (backend.startsWith("native-")) {
          assert.equal(memory.hostHandles, 0); assert.equal(memory.boundaryBuffers, 0);
        } else assert.ok(Object.values(memory).every(n => n === 0));
      }
    } finally { await rm(directory, { recursive: true, force: true }); }
  });
