// Build the private Core/JSPI backend without loading the Component transpiler.
import { execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";
import { resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
const execFile = promisify(execFileCallback);
const root = fileURLToPath(new URL("..", import.meta.url));
export async function build(source, directory, options = {}) {
  const output = resolve(directory);
  await execFile(
    "cargo",
    ["run", "--locked", "--quiet", "--", "build", resolve(source), "--target", "core-js", "-o", output, ...(options.legacySyntax ? ["--legacy-syntax"] : []), ...(options.bindings ? ["--bindings", resolve(options.bindings)] : [])],
    { cwd: root, maxBuffer: 4 * 1024 * 1024 },
  );
  return output;
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const [source, output] = process.argv.slice(2);
  if (!source || !output)
    throw new Error(
      "usage: node runtime/build-core.mjs source.walz output-directory",
    );
  await build(source, output);
}
