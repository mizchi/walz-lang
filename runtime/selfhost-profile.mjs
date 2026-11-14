// The guest performs all expansion and checking, including failed compilations.
import { writeFile, mkdir } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { readProject } from "./selfhost/files.mjs";
import { openCompiler, sourceRecords, recorderRecords, describe } from "./selfhost/profiling.mjs";

const [input, output, compiler = "target/selfhost-profile/compiler"] = process.argv.slice(2);
if (!input || !output) throw new Error("usage: selfhost-profile.mjs input.walz output.json [profile-compiler-directory]");
const { api, artifacts } = await openCompiler(compiler);
const { files, error } = await readProject(api, input);
const start = performance.now();
const profile = error ? { result: error, stats: null } : api.profileProject(files);
const elapsedMs = performance.now() - start;
const report = {
    schema: "walz-selfhost-profile-v4",
  recorder: await recorderRecords(import.meta.url),
  environment: { node: process.version, v8: process.versions.v8, platform: process.platform, arch: process.arch },
  elapsedMs,
  compiler: artifacts,
  sources: sourceRecords(files),
  ...describe(profile.stats),
  ...profile,
};
await mkdir(dirname(resolve(output)), { recursive: true });
await writeFile(output, JSON.stringify(report, null, 2) + "\n");
console.log(`${report.result.tag}: ${report.phase}${report.limit ? ` / ${report.limit}` : ""}; ${elapsedMs.toFixed(2)} ms; ${output}`);
if (report.result.tag === "err") process.exitCode = 1;
