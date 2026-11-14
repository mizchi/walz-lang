// File discovery only; the guest preflight authorizes source forms and paths.
import { readFile } from "node:fs/promises";
import { resolve, dirname } from "node:path";

export async function readProject(api, input) {
  const seen = new Set(), files = [];
  let error;
  async function visit(path) {
    if (seen.has(path) || error) return;
    if (files.length >= 64) {
      error = { tag: "err", val: { file: 0, code: 150, start: 0, end: 0 } };
      return;
    }
    const source = await readFile(path), index = files.length;
    seen.add(path);
    files.push({ path, source });
    const imports = api.imports(source);
    if (imports.tag === "err") {
      error = { tag: "err", val: { ...imports.val, file: index } };
      return;
    }
    for (const { path: specifier } of imports.val) {
      if (!specifier.startsWith("./") && !specifier.startsWith("../")) continue;
      await visit(resolve(dirname(path), specifier));
    }
  }
  await visit(resolve(input));
  return { files, error };
}
