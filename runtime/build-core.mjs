// Build the private Core/JSPI backend without loading the Component transpiler.
import { execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";
import { mkdir, readFile, writeFile, copyFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
const execFile = promisify(execFileCallback);
const root = fileURLToPath(new URL("..", import.meta.url));
export async function build(source, directory, options = {}) {
  const output = resolve(directory);
  await mkdir(output, { recursive: true });
  await execFile(
    "cargo",
    ["run", "--locked", "--quiet", "--", "core-js", resolve(source), output, ...(options.bindings ? ["--bindings", resolve(options.bindings)] : [])],
    { cwd: root, maxBuffer: 4 * 1024 * 1024 },
  );
  const manifest = JSON.parse(
    await readFile(join(output, "manifest.json"), "utf8"),
  );
  if (manifest.abi !== "wlt-core-js-v2")
    throw new Error("unsupported Core JS ABI");
  for (const name of ["core.mjs", "host.mjs", "tasks.mjs", "resources.mjs"])
    await copyFile(new URL(name, import.meta.url), join(output, name));
  await writeFile(
    join(output, "package.json"),
    '{"type":"module","private":true}\n',
  );
  await writeFile(
    join(output, "index.js"),
    `import {instantiate as create} from './core.mjs';
import {bind} from './bindings.mjs';
export {TaskCancelledError} from './tasks.mjs';
export const instantiate=(options={})=>create(import.meta.url,${JSON.stringify(manifest.abi)},bind,options,${JSON.stringify(manifest.imports)});
`,
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
      "usage: node runtime/build-core.mjs source.wlt output-directory",
    );
  await build(source, output);
}
