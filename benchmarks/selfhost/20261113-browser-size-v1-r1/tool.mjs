// Browser-observed deployment sizes. No bundler/minifier/wasm-opt postprocessing.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile, writeFile, mkdir, cp } from "node:fs/promises";
import { dirname, join, resolve, extname, sep } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { cpus } from "node:os";
import { chromium } from "@playwright/test";
import { build } from "./playground-size/build.mjs";
import { deployedPaths, measureFiles, hash } from "./playground-size/metrics.mjs";
import { files, writeJson } from "../runtime/selfhost-bench/artifacts.mjs";
import { NativePeer } from "../runtime/selfhost-bench/peer.mjs";
import { requireEqualOutput } from "../runtime/selfhost-bench/runner.mjs";

async function verify(root) {
  const manifest = JSON.parse(await readFile(join(root, "capture.json")));
  assert.equal(manifest.schema, "walz-playground-size-capture-v1");
  assert.deepEqual((await files(root)).filter(p => p !== "capture.json"), Object.keys(manifest.hashes).sort());
  for (const [path, expected] of Object.entries(manifest.hashes)) {
    assert.equal(hash(await readFile(join(root, path))), expected, `artifact changed: ${path}`);
  }
  return manifest;
}

export async function inspect(root, output) {
  root = resolve(root); output = resolve(output);
  const manifest = await verify(root);
  await mkdir(dirname(output), { recursive: true }); await mkdir(output);
  const cases = await Promise.all(manifest.cases.map(async c => ({ id: c.id,
    source: await readFile(join(root, c.directory, "source.walz"), "utf8"),
    probes: await readFile(join(root, c.directory, "probes.json"), "utf8") })));
  const peer = new NativePeer(join(root, manifest.native));
  const requests = [];
  const server = createServer(async (request, response) => {
    try {
      const uri = new URL(request.url, "http://localhost");
      const pathname = decodeURIComponent(uri.pathname.endsWith("/") ? uri.pathname + "index.html" : uri.pathname);
      const file = resolve(root, "." + pathname);
      if (!file.startsWith(root + sep)) { response.writeHead(403).end(); return; }
      const data = await readFile(file);
      requests.push(uri.pathname);
      const types = { ".wasm": "application/wasm", ".js": "text/javascript", ".mjs": "text/javascript", ".html": "text/html", ".css": "text/css" };
      response.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream", "cache-control": "no-store",
        "cross-origin-opener-policy": "same-origin", "cross-origin-embedder-policy": "require-corp" });
      response.end(data);
    } catch { response.writeHead(404).end(); }
  });
  let browser;
  try {
    await new Promise((ok, fail) => { server.once("error", fail); server.listen(0, "127.0.0.1", ok); });
    browser = await chromium.launch({ headless: true });
    const origin = `http://127.0.0.1:${server.address().port}`;
    const report = { schema: "walz-playground-size-v1", created: new Date().toISOString(), capture: root,
      captureHash: hash(await readFile(join(root, "capture.json"))),
      environment: { node: process.version, cpu: cpus()[0]?.model, platform: process.platform, arch: process.arch,
        chromium: browser.version(), rustc: manifest.rustc, wasmPack: manifest.wasmPack },
      method: { compression: "sum per-resource gzip level 9 and Brotli quality 11, independently compressed; modelled body bytes, excluding HTTP headers/cache effects",
        build: manifest.command, selection: "deduplicated successful HTTP requests under each compiler prefix from fresh Chromium module-worker contexts",
        exclusions: "shared probe HTML/worker, generated program outputs, type declarations, maps, package metadata, unused legacy files and standalone Component build input",
        page: "current Rust playground is separately measured through initial default compile and Run; no full selfhost playground UI or WAT/analysis UI adapter is implemented",
        parity: "8 frozen scalar WAT inputs; exact canonical-module equality with native output, 992 execution/trap probes per compiler, ill-typed input rejected; raw bytes also checked for both Rust targets",
        compatibility: "selfhost requires engine GC and current Core/Component JSPI runtime support; references and selfhost expose different language/tooling features" },
      compilers: {}, playground: {}, buildSourceHashes: manifest.sourceHashes,
      measurementSourceHashes: { "tools/playground-size.mjs": hash(await readFile(fileURLToPath(import.meta.url))) } };
    for (const path of await files(fileURLToPath(new URL("./playground-size/", import.meta.url)))) {
      report.measurementSourceHashes[`tools/playground-size/${path}`] = hash(await readFile(new URL(`./playground-size/${path}`, import.meta.url)));
    }
    for (const backend of ["rust", "wat", "core", "component"]) {
      const context = await browser.newContext();
      try {
        const page = await context.newPage(); await page.goto(origin + "/");
        requests.length = 0;
        const result = await page.evaluate(({ backend, cases }) => new Promise((ok, fail) => {
          const worker = new Worker(new URL("/probe-worker.mjs", location.href), { type: "module" });
          const timeout = setTimeout(() => { worker.terminate(); fail(new Error("compiler probe timeout")); }, 60000);
          const end = () => { clearTimeout(timeout); worker.terminate(); };
          worker.onerror = event => { end(); fail(new Error(event.message)); };
          worker.onmessage = ({ data }) => {
            end();
            if (data.error) fail(new Error(data.error));
            else ok({ ...data, outputs: data.outputs.map(o => ({ id: o.id, wasm: [...o.wasm] })) });
          };
          worker.postMessage({ backend, cases });
        }), { backend, cases });
        const assets = deployedPaths(backend, requests);
        assert.ok(assets.some(p => p.endsWith(".wasm")), `${backend}: no Wasm requested`);
        assert.ok(!assets.some(p => p.endsWith("component.wasm") || p.endsWith(".d.ts")), "unused build artifacts were requested");
        const validation = [];
        for (const output of result.outputs) {
          const { directory } = manifest.cases.find(c => c.id === output.id);
          const normalized = await peer.request({ cmd: "canonical", wasm: output.wasm });
          requireEqualOutput(normalized.canonical, await readFile(join(root, directory, "canonical.wasm")));
          if (backend === "rust" || backend === "wat") requireEqualOutput(output.wasm, await readFile(join(root, directory, "native.wasm")));
          const path = join(outputDir(output.id), `${backend}.wasm`);
          await mkdir(dirname(path), { recursive: true }); await writeFile(path, new Uint8Array(output.wasm));
          validation.push({ id: output.id, bytes: output.wasm.length, canonicalEqual: true });
        }
        assert.equal(result.outputs.length, 8); assert.equal(result.probes, 992); assert.ok(result.illTypedRejected);
        report.compilers[backend] = { ...await measureFiles(root, assets), requests: [...new Set(requests)].sort(),
          validation, probes: result.probes, traps: result.traps, illTypedRejected: result.illTypedRejected };
        console.log(backend, report.compilers[backend].total);
      } finally { await context.close(); }
    }
    // Measure the real existing playground separately, without conflating it with
    // the subset compiler API probes above.
    const context = await browser.newContext();
    try {
      const page = await context.newPage(); requests.length = 0;
      await page.goto(origin + "/rust/");
      await page.getByTestId("compile-status").filter({ hasText: "Compiled successfully" }).waitFor({ timeout: 60000 });
      const initial = deployedPaths("rust", requests);
      await page.getByRole("button", { name: "Run", exact: true }).click();
      await page.getByTestId("run-output").filter({ hasText: "42" }).waitFor({ timeout: 15000 });
      const withRun = deployedPaths("rust", requests);
      report.playground = { initialCompile: await measureFiles(root, initial), throughFirstRun: await measureFiles(root, withRun),
        shellThroughFirstRun: await measureFiles(root, withRun.filter(p => !p.startsWith("rust/pkg/"))),
        runOutput: await page.getByTestId("run-output").textContent() };
    } finally { await context.close(); }
    await verify(root);
    await cp(join(root, "cases"), join(output, "inputs"), { recursive: true });
    await cp(join(root, "capture.json"), join(output, "capture.json"));
    await cp(fileURLToPath(import.meta.url), join(output, "tool.mjs"));
    await cp(new URL("./playground-size/", import.meta.url), join(output, "tool-parts"), { recursive: true });
    await cp(new URL("./wat-web/Cargo.toml", import.meta.url), join(output, "wat-reference.Cargo.toml"));
    await cp(new URL("./wat-web/Cargo.lock", import.meta.url), join(output, "wat-reference.Cargo.lock"));
    await cp(new URL("./wat-web/src/lib.rs", import.meta.url), join(output, "wat-reference.rs"));
    await writeJson(join(output, "results.json"), report);
    return report;
  } catch (error) {
    await writeJson(join(output, "failure.json"), { error: String(error), stack: error.stack });
    throw error;
  } finally {
    await browser?.close(); await peer.close();
    server.closeAllConnections(); await new Promise(ok => server.close(ok));
  }
  function outputDir(id) { return join(output, "outputs", id); }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [mode, first, second] = process.argv.slice(2);
  if (mode === "build" && first) await build(first);
  else if (mode === "measure" && first && second) await inspect(first, second);
  else throw new Error("usage: playground-size.mjs build new-artifacts | measure artifacts new-results");
}
