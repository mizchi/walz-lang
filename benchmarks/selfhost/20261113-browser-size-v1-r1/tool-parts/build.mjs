import assert from "node:assert/strict";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { readFile, writeFile, mkdir, cp, chmod } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build as playground } from "../build-playground.mjs";
import { build as core } from "../../runtime/build-core.mjs";
import { build as component } from "../../runtime/build.mjs";
import { corpus, suite, serializable } from "../../runtime/selfhost-bench/corpus.mjs";
import { verifyCorpus } from "../../runtime/selfhost-bench/capture.mjs";
import { NativePeer } from "../../runtime/selfhost-bench/peer.mjs";
import { files, writeJson } from "../../runtime/selfhost-bench/artifacts.mjs";
import { hash } from "./metrics.mjs";
const root = fileURLToPath(new URL("../../", import.meta.url));
const exec = promisify(callback);

export async function build(output) {
  output = resolve(output);
  const inputs = corpus();
  verifyCorpus(inputs, JSON.parse(await readFile(join(root, "benches/fixtures/selfhost-v2/manifest.json"))));
  await mkdir(dirname(output), { recursive: true }); await mkdir(output);
  // Use the existing playground builder in a clean, isolated directory.
  await playground(join(output, "rust"));
  const reference = await exec("wasm-pack", ["build", "tools/wat-web", "--target", "web", "--release", "--out-dir", join(output, "wat/pkg"), "--out-name", "wat_web", "--locked"],
    { cwd: root, env: { ...process.env, CARGO_TARGET_DIR: join(root, "target") }, maxBuffer: 4 * 1024 * 1024 });
  await writeFile(join(output, "wat-build.log"), reference.stdout + reference.stderr);
  await core("examples/selfhost/portable.walz", join(output, "core"));
  await component("examples/selfhost/portable.walz", join(output, "component"));
  await exec("cargo", ["build", "--locked", "--release", "--example", "selfhost-bench"], { cwd: root, maxBuffer: 4 * 1024 * 1024 });
  const metadata = JSON.parse((await exec("cargo", ["metadata", "--no-deps", "--format-version", "1"], { cwd: root })).stdout);
  const native = "native/selfhost-bench" + (process.platform === "win32" ? ".exe" : "");
  await mkdir(join(output, "native"));
  await cp(join(metadata.target_directory, "release/examples", native.split("/").at(-1)), join(output, native));
  await chmod(join(output, native), 0o755);
  const peer = new NativePeer(join(output, native));
  const cases = [];
  try {
    for (const c of inputs) {
      const dir = `cases/${c.id}`;
      await mkdir(join(output, dir), { recursive: true });
      const expected = await peer.request({ cmd: "prepare", id: c.id, source: c.source });
      await writeFile(join(output, dir, "source.walz"), c.source);
      await writeFile(join(output, dir, "probes.json"), serializable(c.probes));
      await writeFile(join(output, dir, "native.wasm"), new Uint8Array(expected.wasm));
      await writeFile(join(output, dir, "canonical.wasm"), new Uint8Array(expected.canonical));
      cases.push({ id: c.id, directory: dir });
    }
  } finally { await peer.close(); }
  await cp(new URL("./probe-worker.mjs", import.meta.url), join(output, "probe-worker.mjs"));
  await writeFile(join(output, "index.html"), '<!doctype html><meta charset="utf-8"><title>Compiler payload probe</title>');
  const sourceHashes = {};
  for (const directory of ["src", "crates/walz-web", "walz/core", "walz/parser", "walz/checker", "walz/codegen", "tools/playground-size"]) {
    for (const name of await files(join(root, directory))) sourceHashes[`${directory}/${name}`] = hash(await readFile(join(root, directory, name)));
  }
  for (const path of ["Cargo.toml", "Cargo.lock", "build.rs", "tools/build-playground.mjs", "tools/playground-size.mjs", "tools/wat-web/Cargo.toml", "tools/wat-web/Cargo.lock", "tools/wat-web/src/lib.rs", "examples/selfhost/portable.walz", "runtime/build-core.mjs", "runtime/build.mjs"]) {
    sourceHashes[path] = hash(await readFile(join(root, path)));
  }
  const hashes = {};
  for (const name of await files(output)) hashes[name] = hash(await readFile(join(output, name)));
  await writeJson(join(output, "capture.json"), { schema: "walz-playground-size-capture-v1", suite, created: new Date().toISOString(),
    commit: (await exec("git", ["rev-parse", "HEAD"], { cwd: root })).stdout.trim(),
    dirty: (await exec("git", ["status", "--short"], { cwd: root })).stdout,
    rustc: (await exec("rustc", ["--version"])).stdout.trim(), wasmPack: (await exec("wasm-pack", ["--version"])).stdout.trim(),
    command: "wasm-pack --target web --release --locked; wasm-opt disabled in both package manifests; existing Core/Component production builders",
    native, cases, sourceHashes, hashes });
  console.log(`Browser artifacts frozen: ${output}`);
  return output;
}
