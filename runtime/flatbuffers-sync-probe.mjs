// Diagnostic only: changing this option alone need not remove jco async support.
import { transpileBytes } from "@bytecodealliance/jco-transpile";
import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { verify } from "./flatbuffers-bench/runner.mjs";
import { hash } from "./flatbuffers-bench/workloads.mjs";

const [capture, output] = process.argv.slice(2);
if (!capture || !output) throw new Error("usage: flatbuffers-sync-probe.mjs capture-directory output.json");
const directory = resolve(capture); await verify(directory);
const input = await readFile(join(directory, "component/component.wasm"));
const settings = { name: "component", instantiation: "async", wasiShim: false, nodejsCompat: false,
  base64Cutoff: 0, noComponentErrorWrapping: false, bindgenEnableWasmExnref: true, multiMemory: true };
const results = {};
for (const asyncMode of ["jspi", "sync"]) {
  const generated = await transpileBytes(input, { ...settings, asyncMode });
  const bytes = generated.files["component.js"];
  results[asyncMode] = { bytes: bytes.length, sha256: hash(bytes), hasAsyncTask: new TextDecoder().decode(bytes).includes("class AsyncTask") };
}
await writeFile(output, JSON.stringify({ schema: "walz-flatbuffers-sync-probe-v1", node: process.version,
  inputHash: hash(input), settings, jco: "0.23.0", results, byteIdentical: results.jspi.sha256 === results.sync.sha256,
  toolSource: await readFile(new URL(import.meta.url), "utf8") }, null, 2) + "\n", { flag: "wx" });
