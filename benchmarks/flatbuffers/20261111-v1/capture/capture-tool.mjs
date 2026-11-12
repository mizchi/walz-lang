// Build once, then measure captured artifacts without compiler/build activity.
import { mkdir, readFile, writeFile, readdir, copyFile } from "node:fs/promises";
import { join, resolve, dirname, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createRequire } from "node:module";
import { execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";
import { cpus } from "node:os";
import { corpus, materialize, hash } from "./flatbuffers-bench/workloads.mjs";
import { createReference } from "./flatbuffers-bench/reference.mjs";
import { verify } from "./flatbuffers-bench/runner.mjs";
import { instrument } from "./testing/memory.mjs";

const execFile = promisify(execFileCallback), root = fileURLToPath(new URL("..", import.meta.url));
async function files(directory) {
  const out = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    out.push(...entry.isDirectory() ? await files(path) : [path]);
  }
  return out.sort();
}
async function copy(source, target) { await mkdir(dirname(target), { recursive: true }); await copyFile(source, target); }
async function capture(directory) {
  await mkdir(dirname(directory), { recursive: true }); await mkdir(directory);
  const require = createRequire(import.meta.url), referenceRoot = dirname(require.resolve("flatbuffers/package.json"));
  const reference = createReference(await import("flatbuffers")), cases = corpus();
  for (const folder of ["src", "walz/core", "walz/flatbuffers", "examples/flatbuffers"])
    for (const file of await files(join(root, folder))) await copy(file, join(directory, "source", relative(root, file)));
  for (const name of ["Cargo.toml", "Cargo.lock", "build.rs", "package.json", "pnpm-lock.yaml", "LICENSE", "runtime/testing/memory.mjs"])
    await copy(join(root, name), join(directory, "source", name));
  await copy(fileURLToPath(import.meta.url), join(directory, "capture-tool.mjs"));
  for (const name of ["runner.mjs", "workloads.mjs", "reference.mjs"])
    await copy(join(root, "runtime/flatbuffers-bench", name), join(directory, name));
  for (const file of (await files(join(referenceRoot, "js"))).filter(p => p.endsWith(".js")))
    await copy(file, join(directory, "reference-package", relative(referenceRoot, file)));
  for (const name of ["package.json", "LICENSE"]) await copy(join(referenceRoot, name), join(directory, "reference-package", name));
  await mkdir(join(directory, "inputs"));
  for (const c of cases) await writeFile(join(directory, "inputs", c.name + ".bin"), reference.encode(materialize(c.value)), { flag: "wx" });
  await writeFile(join(directory, "corpus.json"), JSON.stringify(cases, null, 2) + "\n", { flag: "wx" });
  for (const backend of ["core", "component"]) {
    const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
    await build(join(root, "examples/flatbuffers/main.walz"), join(directory, backend));
    await instrument(join(directory, backend), backend);
  }
  const command = args => execFile("cargo", args, { cwd: root, maxBuffer: 4 * 1024 * 1024 });
  await command(["run", "--locked", "--quiet", "--", "build", "examples/flatbuffers/resident.walz", join(directory, "resident.wasm")]);
  const expanded = await command(["run", "--locked", "--quiet", "--", "expand", "examples/flatbuffers/resident.walz"]);
  await writeFile(join(directory, "resident-expanded.walz"), expanded.stdout);
  const manifest = {
    schema: "walz-flatbuffers-capture-v1", created: new Date().toISOString(), node: process.version, v8: process.versions.v8, cpu: cpus()[0]?.model,
    compiler: { head: (await execFile("git", ["rev-parse", "HEAD"], { cwd: root })).stdout.trim(),
      dirty: !!(await execFile("git", ["status", "--porcelain"], { cwd: root })).stdout.trim(),
      rustc: (await execFile("rustc", ["--version"])).stdout.trim() },
    reference: { flatbuffers: JSON.parse(await readFile(join(referenceRoot, "package.json"), "utf8")).version, adapter: "handwritten message.fbs adapter using official JS primitives" },
    hashes: {}, sizes: {},
  };
  for (const file of await files(directory)) {
    const bytes = await readFile(file), name = relative(directory, file);
    manifest.hashes[name] = hash(bytes);
    if (name.endsWith(".wasm")) manifest.sizes[name] = bytes.length;
  }
  await writeFile(join(directory, "capture.json"), JSON.stringify(manifest, null, 2) + "\n", { flag: "wx" });
  console.log(directory);
}
const [mode, first, second, third] = process.argv.slice(2);
if (mode === "capture" && first && !second) await capture(resolve(first));
else if (mode === "run" && first && second) {
  const directory = resolve(first); await verify(directory);
  const { run } = await import(pathToFileURL(join(directory, "runner.mjs")));
  await run(directory, resolve(second), { rounds: third === undefined ? 9 : Number(third) });
} else throw new Error("usage: flatbuffers-bench.mjs capture directory | run capture-directory output-directory [rounds]");
