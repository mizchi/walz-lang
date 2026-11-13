import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir, mkdir, writeFile, copyFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { NativePeer } from "./peer.mjs";
import { requireEqualOutput, verifyExecution } from "./runner.mjs";

export const hash = value => createHash("sha256").update(value).digest("hex");
export async function files(directory, prefix = "") {
  const result = [];
  for (const entry of await readdir(join(directory, prefix), { withFileTypes: true })) {
    const path = join(prefix, entry.name);
    if (entry.isDirectory()) result.push(...await files(directory, path));
    else if (entry.isFile()) result.push(path);
    else throw new Error(`unsupported snapshot entry ${path}`);
  }
  return result.sort();
}
export async function copy(source, output, relative) {
  await mkdir(dirname(join(output, relative)), { recursive: true });
  await copyFile(source, join(output, relative));
}
export async function verifySnapshot(directory) {
  const manifest = JSON.parse(await readFile(join(directory, "capture.json"), "utf8"));
  assert.equal(manifest.schema, "walz-selfhost-capture-v1");
  assert.deepEqual((await files(directory)).filter(f => f !== "capture.json"), Object.keys(manifest.hashes).sort(), "snapshot file list changed");
  for (const [file, expected] of Object.entries(manifest.hashes)) {
    assert.equal(hash(await readFile(join(directory, file))), expected, `snapshot changed: ${file}`);
  }
  return manifest;
}
export async function load(directory) {
  const manifest = await verifySnapshot(directory);
  const native = new NativePeer(join(directory, manifest.native));
  const guests = [];
  try {
    for (const backend of ["core", "component"]) {
      const { instantiate } = await import(pathToFileURL(join(directory, backend, "index.js")));
      guests.push({ name: backend, api: (await instantiate()).api });
    }
    const cases = [];
    for (const entry of manifest.cases) {
      const source = await readFile(join(directory, entry.source));
      const probes = JSON.parse(await readFile(join(directory, entry.probes), "utf8"), (_, v) =>
        v && typeof v === "object" && typeof v.i64 === "string" ? BigInt(v.i64) : v);
      const reference = await native.request({ cmd: "prepare", id: entry.id, source: source.toString("utf8") });
      requireEqualOutput(reference.wasm, await readFile(join(directory, entry.native)));
      requireEqualOutput(reference.canonical, await readFile(join(directory, entry.canonical)));
      const outputs = {};
      verifyExecution((await WebAssembly.instantiate(new Uint8Array(reference.wasm))).instance.exports, probes);
      for (const guest of guests) {
        const checked = guest.api.check(source);
        assert.deepEqual(checked, { tag: "ok", val: reference.functions });
        const compiled = guest.api.compile(source);
        assert.equal(compiled.tag, "ok");
        assert.ok(WebAssembly.validate(compiled.val));
        const normalized = await native.request({ cmd: "canonical", wasm: [...compiled.val] });
        requireEqualOutput(normalized.canonical, reference.canonical);
        requireEqualOutput(compiled.val, await readFile(join(directory, entry[guest.name])));
        verifyExecution((await WebAssembly.instantiate(compiled.val)).instance.exports, probes);
        outputs[guest.name] = Uint8Array.from(compiled.val);
      }
      cases.push({ ...entry, source, probes, functions: reference.functions, outputs });
    }
    return { manifest, native, guests, cases };
  } catch (error) { await native.close(); throw error; }
}
export async function writeJson(path, value) {
  await writeFile(path, JSON.stringify(value, null, 2) + "\n", { flag: "wx" });
}
