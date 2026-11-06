import { mkdir, readFile, writeFile, readdir, copyFile, stat } from "node:fs/promises";
import { join, resolve, relative, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";
import { gzipSync } from "node:zlib";
import { performance } from "node:perf_hooks";
import { cpus } from "node:os";
import { hash, verify } from "./simd-kernels/runner.mjs";
import { build as buildCore } from "./build-core.mjs";
import { build as buildComponent } from "./build.mjs";

const execFile = promisify(execFileCallback);
const root = fileURLToPath(new URL("..", import.meta.url));
async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await files(path));
    else if (entry.isFile()) result.push(path);
  }
  return result.sort();
}
async function copy(source, target) {
  await mkdir(dirname(target), { recursive: true });
  await copyFile(source, target);
}
async function git(directory) {
  const { stdout: head } = await execFile("git", ["rev-parse", "HEAD"], { cwd: directory });
  const { stdout: status } = await execFile("git", ["status", "--porcelain"], { cwd: directory });
  return { head: head.trim(), dirty: Boolean(status.trim()) };
}
async function capture(directory, reference) {
  // Check reference binaries before creating a permanent snapshot directory.
  const features = ["i32-array", "f32-vector", "bitmap", "bytes"];
  for (const feature of features) await stat(join(reference, "packages/jsimd/src", feature, "kernels.wasm"));
  await mkdir(dirname(directory), { recursive: true });
  await mkdir(directory); // Captures are immutable; never overwrite an old run.
  for (const folder of ["src", "stdlib", "examples/simd-kernels"])
    for (const path of await files(join(root, folder))) await copy(path, join(directory, "source", relative(root, path)));
  for (const name of ["Cargo.toml", "Cargo.lock"]) await copy(join(root, name), join(directory, "source", name));
  for (const name of ["runner.mjs", "workloads.mjs"]) await copy(join(root, "runtime/simd-kernels", name), join(directory, name));
  await copy(fileURLToPath(import.meta.url), join(directory, "capture-tool.mjs"));
  await copy(join(reference, "LICENSE"), join(directory, "reference/LICENSE"));
  for (const feature of features)
    for (const name of ["kernels.wasm", feature === "bytes" ? "kernels.wat" : "kernels.zig", "README.md"])
      await copy(join(reference, "packages/jsimd/src", feature, name), join(directory, "reference", feature, name));
  const start = performance.now();
  await execFile("cargo", ["run", "--locked", "--quiet", "--", "build", "examples/simd-kernels/resident.wlt", join(directory, "resident.wasm")], { cwd: root });
  await buildCore(join(root, "examples/simd-kernels/portable.wlt"), join(directory, "core"));
  await buildComponent(join(root, "examples/simd-kernels/portable.wlt"), join(directory, "component"));
  const sizesBytes = {};
  for (const path of await files(directory)) if (path.endsWith(".wasm")) {
    const bytes = await readFile(path);
    sizesBytes[relative(directory, path)] = { raw: bytes.length, gzip: gzipSync(bytes).length };
  }
  const manifest = {
    schema: "wlt-simd-capture-v1", created: new Date().toISOString(),
    node: process.version, cpu: cpus()[0]?.model, compiler: await git(root),
    reference: { ...await git(reference), description: "local jsimd checkout, captured generated binaries; raw kernels, no public JS wrappers", upstream: "https://github.com/mizchi/jsimd" },
    buildMs: performance.now() - start, sizesBytes, hashes: {},
  };
  for (const path of await files(directory)) manifest.hashes[relative(directory, path)] = hash(await readFile(path));
  await writeFile(join(directory, "capture.json"), JSON.stringify(manifest, null, 2) + "\n", { flag: "wx" });
  console.log(directory);
}

const [mode, first, second, third] = process.argv.slice(2);
if (mode === "capture" && first && second && !third) await capture(resolve(first), resolve(second));
else if (mode === "run" && first) {
  const directory = resolve(first);
  await verify(directory); // Verify the stored runner before importing it.
  const { run } = await import(pathToFileURL(join(directory, "runner.mjs")));
  await run(directory, { rounds: second === undefined ? 9 : Number(second), sizes: third?.split(",").map(Number) });
} else throw new Error("usage: simd-kernels-bench.mjs capture directory jsimd-checkout | run directory [rounds] [comma-separated-sizes]");
