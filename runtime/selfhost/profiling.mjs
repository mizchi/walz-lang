import { readFile, readdir } from "node:fs/promises";
import { resolve, join, relative } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

export const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
export const sourceRecords = (files) => files.map(({ path, source }) => ({
  path: relative(process.cwd(), path), sha256: sha256(source),
}));

export function describe(stats) {
  const direct = stats && "syntaxNodes" in stats;
  const phases = ["not-started", "link", "generics", "gc-order", direct ? "lower" : "lower-render", direct ? "syntax" : "reparse", "validate", "complete"];
  const limits = [null, "instances", "elaboration-work", "traversal-depth", "gc-types", "named-fields", "gc-edges", "rendered-source", "syntax-nodes", "syntax-storage", "syntax-work"];
  const rate = (name) => stats?.[name + "Requests"] ? stats[name + "Hits"] / stats[name + "Requests"] : null;
  return {
    countersSchema: stats && "syntaxAtomHits" in stats ? "walz-selfhost-stats-v4" : direct ? "walz-selfhost-stats-v3" : stats && "layoutRequests" in stats ? "walz-selfhost-stats-v2" : stats ? "walz-selfhost-stats-v1" : null,
    phase: stats ? phases[stats.phase] : "discovery",
    limit: stats ? limits[stats.limit] : null,
    cacheHitRates: Object.fromEntries(["signature", "return", "instance", "alias", "layout", "field"].map((name) => [name, rate(name)])),
  };
}

export async function openCompiler(path) {
  const directory = resolve(path);
  const { api } = await (await import(pathToFileURL(join(directory, "index.js")))).instantiate();
  const artifacts = [];
  async function inventory(root) {
    for (const entry of await readdir(root, { withFileTypes: true })) {
      const path = join(root, entry.name);
      if (entry.isDirectory()) await inventory(path);
      else if (entry.isFile() && (/\.(wasm|mjs|js)$/.test(entry.name) || ["manifest.json", "bindings.json", "package.json"].includes(entry.name)))
        artifacts.push({ path: relative(directory, path), sha256: sha256(await readFile(path)) });
    }
  }
  await inventory(directory);
  artifacts.sort((a, b) => a.path.localeCompare(b.path));
  return { api, artifacts };
}

export async function recorderRecords(entry) {
  const urls = [new URL(entry), new URL("./profiling.mjs", import.meta.url), new URL("./files.mjs", import.meta.url)];
  return Promise.all(urls.map(async (url) => ({
    path: relative(process.cwd(), fileURLToPath(url)),
    sha256: sha256(await readFile(url)),
  })));
}
