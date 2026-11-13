import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { gzipSync, brotliCompressSync, constants } from "node:zlib";
import { measureFiles, deployedPaths } from "../tools/playground-size/metrics.mjs";

test("browser size counts each requested compiler resource once and ignores query strings", () => {
  assert.deepEqual(deployedPaths("core", ["/probe-worker.mjs", "/core/index.js", "/core/core.wasm?v=1", "/core/core.wasm?v=2", "/rust/pkg/walz_web.js"]), ["core/core.wasm", "core/index.js"]);
  assert.throws(() => deployedPaths("core", ["/core/%2e%2e/private.wasm"]), /path/);
  assert.deepEqual(deployedPaths("rust", ["/rust/", "/rust/index.html?cache=1"]), ["rust/index.html"]);
});

test("deployment size sums independently compressed fetched assets, excluding stale files and build inputs", async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-browser-size-"));
  try {
    await mkdir(join(directory, "core"));
    const a = Buffer.from("one\n".repeat(200)), b = Buffer.from("two\n".repeat(300));
    await writeFile(join(directory, "core/a.js"), a);
    await writeFile(join(directory, "core/b.wasm"), b);
    await writeFile(join(directory, "core/unused-component.wasm"), new Uint8Array(9999));
    const result = await measureFiles(directory, ["core/a.js", "core/b.wasm"]);
    assert.equal(result.total.raw, a.length + b.length);
    assert.equal(result.total.gzip, gzipSync(a, {level: 9}).length + gzipSync(b, {level: 9}).length);
    assert.equal(result.total.brotli, [a, b].reduce((n, bytes) => n + brotliCompressSync(bytes, {params: {[constants.BROTLI_PARAM_QUALITY]: 11}}).length, 0));
    assert.equal(result.wasm.raw, b.length);
    assert.equal(result.js.raw, a.length);
    assert.equal(result.assets.length, 2);
    assert.ok(result.assets.every(v => v.sha256.length === 64));
    await assert.rejects(measureFiles(directory, ["../escape"]), /path/);
  } finally { await rm(directory, {recursive: true, force: true}); }
});
