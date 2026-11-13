// Build/freeze inputs and compiler artifacts separately from measurement.
import assert from "node:assert/strict";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { readFile, mkdir, writeFile, cp, chmod } from "node:fs/promises";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { corpus, suite, serializable } from "./corpus.mjs";
import { NativePeer } from "./peer.mjs";
import { hash, files, copy, writeJson, load, verifySnapshot } from "./artifacts.mjs";
const exec = promisify(callback);
const root = fileURLToPath(new URL("../../", import.meta.url));

export async function capture(output) {
  output = resolve(output);
  await mkdir(dirname(output), { recursive: true });
  await mkdir(output); // Never replace recorded data, including partial captures.
  const metadata = JSON.parse((await exec("cargo", ["metadata", "--no-deps", "--format-version", "1"], { cwd: root })).stdout);
  const built = await exec("cargo", ["build", "--locked", "--release", "--example", "selfhost-bench", "--bin", "walz"], { cwd: root, maxBuffer: 4 * 1024 * 1024 });
  await writeFile(join(output, "build.log"), built.stderr);
  const executable = join(metadata.target_directory, "release/examples/selfhost-bench" + (process.platform === "win32" ? ".exe" : ""));
  const nativePath = "native/selfhost-bench" + (process.platform === "win32" ? ".exe" : "");
  await copy(executable, output, nativePath);
  await chmod(join(output, nativePath), 0o755);
  const source = join(root, "examples/selfhost/portable.walz");
  await (await import("../build-core.mjs")).build(source, join(output, "core"));
  await (await import("../build.mjs")).build(source, join(output, "component"));
  const guests = [];
  for (const backend of ["core", "component"]) {
    const { instantiate } = await import(pathToFileURL(join(output, backend, "index.js")));
    guests.push({ name: backend, api: (await instantiate()).api });
  }
  const native = new NativePeer(join(output, nativePath));
  const cases = [];
  try {
    for (const c of corpus()) {
      const dir = `cases/${c.id}`;
      await mkdir(join(output, dir), { recursive: true });
      await writeFile(join(output, dir, "source.walz"), c.source);
      await writeFile(join(output, dir, "probes.json"), serializable(c.probes));
      const reference = await native.request({ cmd: "prepare", id: c.id, source: c.source });
      await writeFile(join(output, dir, "native.wasm"), new Uint8Array(reference.wasm));
      await writeFile(join(output, dir, "canonical.wasm"), new Uint8Array(reference.canonical));
      const gate = {};
      for (const guest of guests) {
        const compiled = guest.api.compile(new TextEncoder().encode(c.source));
        assert.equal(compiled.tag, "ok", c.id);
        const { canonical } = await native.request({ cmd: "canonical", wasm: [...compiled.val] });
        assert.deepEqual(canonical, reference.canonical, `${c.id}: canonical output mismatch`);
        await writeFile(join(output, dir, `${guest.name}.wasm`), compiled.val);
        gate[guest.name] = { canonicalEqual: true, rawEqual: Buffer.from(reference.wasm).equals(compiled.val), bytes: compiled.val.length };
      }
      cases.push({ id: c.id, source: `${dir}/source.walz`, probes: `${dir}/probes.json`, native: `${dir}/native.wasm`,
        canonical: `${dir}/canonical.wasm`, core: `${dir}/core.wasm`, component: `${dir}/component.wasm`,
        inputBytes: Buffer.byteLength(c.source), nativeBytes: reference.wasm.length, structuredCheck: reference.structuredCheck, gate });
      console.log(`Captured ${c.id}: canonical equality passed`);
    }
  } finally { await native.close(); }
  for (const directory of ["src", "walz/core", "walz/parser", "walz/checker", "walz/codegen", "runtime/selfhost-bench", "tools/selfhost"]) {
    await cp(join(root, directory), join(output, "source", directory), { recursive: true });
  }
  for (const file of ["Cargo.toml", "Cargo.lock", "build.rs", "examples/selfhost/portable.walz", "tools/selfhost-bench.rs", "runtime/selfhost-bench.mjs", "runtime/build-core.mjs", "runtime/build.mjs"]) {
    await copy(join(root, file), output, `source/${file}`);
  }
  const hashes = {};
  for (const file of await files(output)) hashes[file] = hash(await readFile(join(output, file)));
  await writeJson(join(output, "capture.json"), { schema: "walz-selfhost-capture-v1", suite, created: new Date().toISOString(),
    commit: (await exec("git", ["rev-parse", "HEAD"], { cwd: root })).stdout.trim(),
    dirty: (await exec("git", ["status", "--short"], { cwd: root })).stdout,
    rustc: (await exec("rustc", ["--version"], { cwd: root })).stdout.trim(), native: nativePath, cases, hashes });
  const checked = await load(output);
  await checked.native.close();
  await verifySnapshot(output);
  console.log(`Verified capture: ${relative(root, output)}`);
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [output] = process.argv.slice(2);
  if (!output) throw new Error("usage: capture.mjs new-output-directory");
  await capture(output);
}
