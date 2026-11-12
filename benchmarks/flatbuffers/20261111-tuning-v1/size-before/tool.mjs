import assert from "node:assert/strict";
import { readFile, writeFile, readdir, mkdir, copyFile, cp } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { gzipSync, brotliCompressSync, constants } from "node:zlib";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { verify } from "./flatbuffers-bench/runner.mjs";
import { hash, materialize } from "./flatbuffers-bench/workloads.mjs";
import { createReference } from "./flatbuffers-bench/reference.mjs";
import { wasmSections } from "./flatbuffers-inspect.mjs";

const exec = promisify(callback);
const save = (path, value) => writeFile(path, JSON.stringify(value, null, 2) + "\n", { flag: "wx" });
const sizes = bytes => ({ raw: bytes.length, gzip: gzipSync(bytes, { level: 9 }).length,
  brotli: brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length, sha256: hash(bytes) });
export async function bundle(entry, name, output) {
  const { build } = await import("vite");
  const path = join(output, `${name}.entry.mjs`);
  await writeFile(path, entry, { flag: "wx" });
  const result = await build({ configFile: false, publicDir: false, logLevel: "error", root: output,
    build: { write: false, target: "esnext", minify: "oxc", sourcemap: true, lib: { entry: path, formats: ["es"] },
      commonjsOptions: { include: [/reference-package/, /node_modules/] } } });
  const chunks = (Array.isArray(result) ? result : [result]).flatMap(r => r.output).filter(c => c.type === "chunk");
  const folder = name + "-js"; await mkdir(join(output, folder));
  const assets = {}, modules = [], total = { raw: 0, gzip: 0, brotli: 0 };
  for (const chunk of chunks) {
    await writeFile(join(output, folder, chunk.fileName), chunk.code, { flag: "wx" });
    if (chunk.map) await writeFile(join(output, folder, chunk.fileName + ".map"), chunk.map.toString(), { flag: "wx" });
    assets[chunk.fileName] = sizes(Buffer.from(chunk.code));
    for (const key of Object.keys(total)) total[key] += assets[chunk.fileName][key];
    for (const [path, m] of Object.entries(chunk.modules)) modules.push({ path, renderedLength: m.renderedLength, originalLength: m.originalLength, removedExports: m.removedExports });
  }
  const entryChunk = chunks.find(c => c.isEntry); assert.ok(entryChunk);
  return { ...total, entry: join(folder, entryChunk.fileName), assets, modules: modules.sort((a, b) => b.renderedLength - a.renderedLength) };
}
export async function inspect(directory, output, optimizer) {
  await verify(directory);
  const version = (await exec(optimizer, ["--version"])).stdout.trim();
  assert.match(version, /\b133\b/, "select Binaryen 133 explicitly");
  await mkdir(output);
  await copyFile(fileURLToPath(import.meta.url), join(output, "tool.mjs"));
  const reference = createReference(await import(pathToFileURL(join(directory, "reference-package/js/flatbuffers.js"))));
  const corpus = JSON.parse(await readFile(join(directory, "corpus.json"), "utf8"));
  const source = JSON.stringify(join(directory, "reference-package/js/flatbuffers.js"));
  const helper = JSON.stringify(join(directory, "reference.mjs"));
  const report = { schema: "walz-flatbuffers-size-v1", node: process.version, created: new Date().toISOString(),
    captureHash: hash(await readFile(join(directory, "capture.json"))), optimizer: { path: optimizer, version, sha256: hash(await readFile(optimizer)) },
    method: "Vite ES library, oxc minification, esnext; gzip level 9 and Brotli quality 11. Sum independently compressed deployed JS/Wasm assets. Inlined Wasm is counted inside JS only. Rebundle each optimized variant. Maps and unused external files are excluded. Source module attribution is pre-minifier rendered length.",
    bundles: {}, wasm: {}, validation: [] };
  report.bundles.js = await bundle(`import {Builder,ByteBuffer} from ${source};\nimport {createReference} from ${helper};\nexport const {encode,decode,createEncoder}=createReference({Builder,ByteBuffer});\n`, "reference", output);
  const bundledReference = await import(pathToFileURL(join(output, report.bundles.js.entry)));
  for (const c of corpus) assert.deepEqual(bundledReference.decode(bundledReference.encode(materialize(c.value))), materialize(c.value));
  for (const backend of ["core", "component"]) {
    const names = (await readdir(join(directory, backend))).filter(n => n.endsWith(".wasm") && n !== "component.wasm");
    report.wasm[backend] = {};
    for (const mode of ["original", "strip", "O3", "Oz"]) {
      const modules = new Map(), assets = {};
      const variant = backend + "-" + mode, assetDirectory = join(output, variant);
      await cp(join(directory, backend), assetDirectory, { recursive: true });
      for (const name of names) {
        const input = join(directory, backend, name), target = join(assetDirectory, name), original = await readFile(input);
        const flags = ["--enable-gc", "--enable-reference-types", "--enable-exception-handling", "--enable-bulk-memory",
          "--enable-mutable-globals", "--enable-multivalue", "--enable-multimemory", "--enable-simd",
          "--strip-debug", "--strip-producers", "--strip-target-features", ...(mode === "strip" ? [] : ["-" + mode])];
        if (mode === "original") await copyFile(input, target);
        else await exec(optimizer, [input, ...flags, "-o", target], { maxBuffer: 2 * 1024 * 1024 });
        const bytes = await readFile(target);
        assets[name] = { ...sizes(bytes), sections: wasmSections(bytes), ...(mode === "original" ? {} : { command: [optimizer, input, ...flags, "-o", target] }) };
        modules.set(name, await WebAssembly.compile(bytes));
        assert.ok(original.length > 0);
      }
      const bundled = await bundle(`export {instantiate} from ${JSON.stringify(join(assetDirectory, "index.js"))};\n`, variant, output);
      if (mode === "original") report.bundles[backend] = bundled;
      const { instantiate } = await import(pathToFileURL(join(output, bundled.entry)));
      const { api } = await instantiate({ getCoreModule: async name => {
        assert.ok(modules.has(name), `unexpected Wasm asset ${name}`); return modules.get(name);
      } });
      for (const c of corpus) {
        const value = materialize(c.value), bytes = new Uint8Array(await readFile(join(directory, "inputs", c.name + ".bin")));
        assert.deepEqual(reference.decode(api.encode(value)), value);
        assert.deepEqual(api.decode(bytes), { tag: "ok", val: value });
      }
      assert.deepEqual(api.decode(new Uint8Array()), { tag: "err", val: 1 });
      const total = { raw: 0, gzip: 0, brotli: 0 };
      const inlined = names.filter(name => bundled.modules.some(m => m.path.endsWith("/" + name + "?url")));
      for (const asset of [bundled, ...Object.entries(assets).filter(([name]) => !inlined.includes(name)).map(([, a]) => a)])
        for (const key of Object.keys(total)) total[key] += asset[key];
      report.wasm[backend][mode] = { assets, bundle: bundled, inlinedWasm: inlined, deployedTotal: total };
      report.validation.push(`${backend}/${mode}: bundled JS + Wasm, all four inputs + malformed input`);
    }
  }
  await save(join(output, "size.json"), report);
  for (const [name, row] of Object.entries(report.bundles)) console.log(`${name} JS: ${row.raw} raw / ${row.gzip} gzip bytes`);
  for (const [name, variants] of Object.entries(report.wasm)) for (const [mode, row] of Object.entries(variants))
    console.log(`${name}/${mode} deployed: ${row.deployedTotal.raw} raw / ${row.deployedTotal.gzip} gzip bytes`);
  return report;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [snapshot, output, optimizer] = process.argv.slice(2);
  if (!snapshot || !output || !optimizer) throw new Error("usage: flatbuffers-size.mjs capture-directory output-directory binaryen-133-wasm-opt");
  await inspect(resolve(snapshot), resolve(output), resolve(optimizer));
}
