import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { build } from "./build.mjs";
import { bundle } from "./flatbuffers-size.mjs";
import { hash } from "./flatbuffers-bench/workloads.mjs";

test("Component browser bundles reference only the generated core modules", { timeout: 60000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-component-bundle-"));
  try {
    const source = join(directory, "source"), output = join(directory, "bundle");
    await mkdir(output);
    await build("examples/flatbuffers/main.walz", source);
    await writeFile(join(source, "unrelated-file.txt"), "must-not-be-bundled");
    const result = await bundle(`export {instantiate} from ${JSON.stringify(join(source, "index.js"))};`, "codec", output);
    const urlAssets = result.modules.filter(m => m.path.endsWith("?url")).map(m => m.path.split("/").at(-1));
    assert.deepEqual(urlAssets.filter(name => !["component.core.wasm?url", "component.core2.wasm?url"].includes(name)), []);
    assert.deepEqual([...new Set(result.inlinedAssetHashes)].sort(),
      (await Promise.all(["component.core.wasm", "component.core2.wasm"].map(async name => hash(await readFile(join(source, name)))))).sort());
    for (const name of Object.keys(result.assets)) assert.ok(!(await readFile(join(output, "codec-js", name), "utf8")).includes("must-not-be-bundled"));
    const { instantiate } = await import(pathToFileURL(join(output, result.entry)));
    const { api } = await instantiate({ getCoreModule: async name => WebAssembly.compile(await readFile(join(source, name))) });
    assert.equal(api.main(), 42);
    assert.deepEqual(api.decode(new Uint8Array()), { tag: "err", val: 1 });
  } finally { await rm(directory, { recursive: true, force: true }); }
});
