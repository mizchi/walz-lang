import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import {
  checkInputs,
  sha256,
  projectRoot,
  implementationFingerprint,
  toolVersions,
} from "./runs.mjs";
import { assertSnapshotIncludes } from "./source.mjs";
import { verifyStock } from "../stock-ledger/verify.mjs";
const exec = promisify(rawExec);
const self = fileURLToPath(import.meta.url);
const save = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + "\n");
async function sourceHashes(directory) {
  const manifest = JSON.parse(
    await readFile(join(directory, "manifest.json"), "utf8"),
  );
  if (manifest.suite.id === "diagnostics")
    return {
      candidateHashes: await (
        await import("../diagnostics/runs.mjs")
      ).candidateHashes(directory),
    };
  return {
    candidateSha256: sha256(
      await readFile(join(directory, "writer/candidate.wlt")),
    ),
    sampleSha256: sha256(
      await readFile(join(directory, "reader/counted.wlt")),
    ),
  };
}
export async function verifyRun(directory, { timeoutMs = 30000 } = {}) {
  directory = resolve(directory);
  if (!Number.isInteger(timeoutMs) || timeoutMs < 100 || timeoutMs > 300000)
    throw new Error("timeout must be 100..300000 ms");
  const destination = join(directory, "verification.json");
  try {
    await exec(process.execPath, [self, "worker", directory], {
      cwd: projectRoot,
      timeout: timeoutMs,
      maxBuffer: 1024 * 1024,
    });
  } catch (e) {
    // A killed worker must never leave an earlier successful report current.
    const hashes = await sourceHashes(directory).catch(() => ({}));
    const inputsFingerprint = await checkInputs(directory).catch(() => null);
    await save(destination, {
      passed: false,
      implementation: await implementationFingerprint(),
      node: process.version,
      ...hashes,
      inputsFingerprint,
      fatal: e.killed ? `evaluation timed out after ${timeoutMs}ms` : e.message,
      subjects: {},
    });
  }
  return JSON.parse(await readFile(destination, "utf8"));
}
async function worker(directory) {
  const destination = join(directory, "verification.json");
  const manifestInfo = JSON.parse(
    await readFile(join(directory, "manifest.json"), "utf8"),
  );
  if (manifestInfo.suite.id === "diagnostics")
    return (await import("../diagnostics/runs.mjs")).verifyDiagnostics(
      directory,
    );
  const hashes = await sourceHashes(directory);
  try {
    const inputsFingerprint = await checkInputs(directory);
    const manifest = JSON.parse(
      await readFile(join(directory, "manifest.json"), "utf8"),
    );
    const implementation = await implementationFingerprint();
    const subjects = {};
    for (const [name, path] of [
      ["sample", "reader/counted.wlt"],
      ["candidate", "writer/candidate.wlt"],
    ]) {
      const source = join(directory, path);
      assertSnapshotIncludes(await readFile(source, "utf8"), manifest.suite.version >= 13 ? 'wlt:core' : 'wlt:std');
      subjects[name] = await verifyStock({
        source,
        output: join(directory, "work", name),
        checkInputs: () => checkInputs(directory),
        groupedBindings: manifest.suite.version >= 7,
        extended: name === "candidate" && manifest.suite.version >= 8,
        legacySyntax: manifest.suite.version < 14,
      });
    }
    if (
      JSON.stringify(hashes) !== JSON.stringify(await sourceHashes(directory))
    )
      throw new Error("evaluation source changed during execution");
    await checkInputs(directory);
    await save(destination, {
      schema: "wlt-eval-verification-v1",
      passed: Object.values(subjects).every((s) => s.passed),
      inputsFingerprint,
      ...hashes,
      subjects,
      implementation,
      toolchain: await toolVersions(),
      implementationChangedSincePrepare:
        implementation.sha256 !== manifest.implementation.sha256,
      node: process.version,
    });
  } catch (e) {
    await save(destination, {
      passed: false,
      implementation: await implementationFingerprint(),
      node: process.version,
      ...hashes,
      inputsFingerprint: await checkInputs(directory).catch(() => null),
      fatal: e.message,
      subjects: {},
    });
  }
}

// Old archived grammar-eval runs retain their original 14-check output format.
export async function verifyLegacy(directory, sourceOverride) {
  directory = resolve(directory);
  try {
    await readFile(join(directory, "manifest.json"));
    if (!sourceOverride) return verifyRun(directory);
  } catch (e) {
    if (e.code !== "ENOENT") throw e;
  }
  const destination = join(
    directory,
    sourceOverride ? "negative-control.json" : "verification.json",
  );
  try {
    await exec(
      process.execPath,
      [
        self,
        "legacy",
        directory,
        ...(sourceOverride ? [resolve(sourceOverride)] : []),
      ],
      { cwd: projectRoot, timeout: 30000, maxBuffer: 1024 * 1024 },
    );
  } catch (e) {
    await save(destination, { passed: false, fatal: e.message });
  }
  return JSON.parse(await readFile(destination, "utf8"));
}
if (process.argv[1] && resolve(process.argv[1]) === self) {
  const [mode, directory, sourceOverride] = process.argv.slice(2);
  if (mode === "worker") await worker(directory);
  else if (mode === "legacy") {
    const source = sourceOverride ?? join(directory, "writer/candidate.gwat");
    const result = await verifyStock({
      source,
      legacySyntax: true,
      output: join(
        projectRoot,
        "target/evals/legacy",
        sha256(directory + String(sourceOverride)).slice(0, 16),
      ),
      checkInputs: () => checkInputs(directory),
    });
    await save(
      join(
        directory,
        sourceOverride ? "negative-control.json" : "verification.json",
      ),
      result,
    );
  } else throw new Error("internal evaluator worker mode required");
}
