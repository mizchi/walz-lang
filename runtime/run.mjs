import { readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
const [directory, exportName = "main", ...arguments_] = process.argv.slice(2);
if (!directory)
  throw new Error(
    "usage: node runtime/run.mjs output-directory [export] [JSON arguments...]",
  );
const output = resolve(directory);
const { instantiate } = await import(pathToFileURL(join(output, "index.js")));
const { api } = await instantiate({
  getCoreModule: async (name) =>
    WebAssembly.compile(await readFile(join(output, name))),
});
const result = await api[exportName](
  ...arguments_.map((value) =>
    JSON.parse(value, (_, v) =>
      typeof v === "string" && /^-?\d+n$/.test(v) ? BigInt(v.slice(0, -1)) : v,
    ),
  ),
);
console.log(
  typeof result === "bigint" ? result.toString() : JSON.stringify(result),
);
