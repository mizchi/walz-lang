import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join, posix } from "node:path";
import { gzipSync, brotliCompressSync, constants } from "node:zlib";
import { createHash } from "node:crypto";

export const hash = bytes => createHash("sha256").update(bytes).digest("hex");
function safe(path) {
  assert.ok(path && !path.startsWith("/") && !path.includes("\\") && path.split("/").every(p => p !== "." && p !== ".."), `invalid asset path: ${path}`);
  return path;
}
export function deployedPaths(name, requests) {
  const prefix = `/${safe(name)}/`;
  return [...new Set(requests.map(r => decodeURIComponent(r.split(/[?#]/)[0]))
    .filter(p => p.startsWith(prefix)).map(p => safe((p.endsWith("/") ? p + "index.html" : p).slice(1))))].sort();
}
export function compressed(bytes) {
  return { raw: bytes.length, gzip: gzipSync(bytes, { level: 9 }).length,
    brotli: brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length };
}
const zero = () => ({ raw: 0, gzip: 0, brotli: 0 });
export async function measureFiles(root, paths) {
  const total = zero(), wasm = zero(), js = zero(), other = zero(), assets = [];
  for (const path of [...new Set(paths)].sort()) {
    const bytes = await readFile(join(root, safe(path)));
    const size = compressed(bytes);
    const type = posix.extname(path) === ".wasm" ? "wasm" : /\.(m?js)$/.test(path) ? "js" : "other";
    for (const kind of Object.keys(total)) {
      total[kind] += size[kind];
      ({ wasm, js, other })[type][kind] += size[kind];
    }
    assets.push({ path, type, ...size, sha256: hash(bytes) });
  }
  return { total, wasm, js, other, assets };
}
