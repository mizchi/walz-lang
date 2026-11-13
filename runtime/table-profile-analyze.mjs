import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { modelCompoundBlocks, hash, verifySnapshot } from "./table-profile-summary.mjs";

export async function analyze(directory) {
  const profile = JSON.parse(await readFile(join(directory, "profile.json")));
  await verifySnapshot(profile.snapshot);
  const bytes = await readFile(join(profile.snapshot, "inputs/262144-numeric.json"));
  const input = JSON.parse(bytes);
  input.x = input.x.map(v => v?.tag === "undefined" ? undefined : v?.tag === "NaN" ? NaN : v);
  return { schema: "walz-table-profile-analysis-v1", profileSha256: hash(await readFile(join(directory, "profile.json"))),
    inputSha256: hash(bytes), blockModel: modelCompoundBlocks(input), cases: profile.cases.map(c => ({
      name: c.name, medianUs: c.timing.medianUs,
      acornSelfPercent: c.cpu.functions.filter(f => f.url.includes("/acorn/")).reduce((a, f) => a + f.selfPercent, 0),
      inspectorSelfPercent: c.cpu.functions.filter(f => f.url === "node:inspector").reduce((a, f) => a + f.selfPercent, 0),
      bridgeSelfPercent: c.cpu.functions.filter(f => f.name.startsWith("js-to-wasm:")).reduce((a, f) => a + f.selfPercent, 0),
      gcSelfPercent: c.cpu.functions.filter(f => f.name === "(garbage collector)").reduce((a, f) => a + f.selfPercent, 0),
    })) };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [directory, output] = process.argv.slice(2);
  if (!directory || !output) throw new Error("usage: table-profile-analyze.mjs profile-directory new-output.json");
  await writeFile(output, JSON.stringify(await analyze(resolve(directory)), null, 2) + "\n", { flag: "wx" });
}
