import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdir } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
export async function build(directory) {
  directory = resolve(directory); await mkdir(directory, { recursive: true });
  await promisify(execFile)("cargo", ["run", "--locked", "--quiet", "--", "build", "examples/table/kernel.walz", join(directory, "core.wasm")], { cwd: root, maxBuffer: 16 * 1024 * 1024 });
  return directory;
}
if (process.argv[1] === fileURLToPath(import.meta.url)) await build(process.argv[2] ?? "target/table");
