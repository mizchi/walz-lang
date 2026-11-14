// File I/O and bootstrap module loading only. Parsing/checking/encoding run in Walz.
import { readFile, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { readProject } from "./selfhost/files.mjs";
import { diagnosticMessages as messages } from "./selfhost/diagnostics.mjs";

const args = process.argv.slice(2);
const checkOnly = args[0] === "--check";
const [input, output, directory = "target/selfhost"] = checkOnly
  ? [args[1], undefined, args[2] ?? "target/selfhost"]
  : args;
if (!input || (!checkOnly && !output)) {
  throw new Error(
    "usage: selfhost.mjs input.walz output.wasm [compiler-directory] | --check input.walz [compiler-directory]",
  );
}
const { instantiate } = await import(
  pathToFileURL(join(resolve(directory), "index.js"))
);
const { api } = await instantiate();
let sourceFiles;
let result;
if (
  typeof api.imports === "function" &&
  typeof api.compileProject === "function"
) {
  const { files, error: inputError } = await readProject(api, input);
  sourceFiles = files;
  const source = files[0].source;
  result =
    inputError ??
    (sourceFiles.length > 1
      ? checkOnly
        ? api.checkProject(sourceFiles)
        : api.compileProject(sourceFiles)
      : checkOnly
        ? api.check(source)
        : api.compile(source));
} else {
  // Frozen compiler captures retain their original single-file API.
  const source = await readFile(input);
  result = checkOnly ? api.check(source) : api.compile(source);
}
if (result.tag === "err") {
  const { code, start, end } = result.val;

  console.error(
    `${result.val.file === -1 ? "walz:core" : (sourceFiles?.[result.val.file]?.path ?? input)}:${start}..${end}: bootstrap compiler error ${code}${messages[code] ? `: ${messages[code]}` : ""}`,
  );
  process.exitCode = 1;
} else if (checkOnly) {
  console.log(`Checked ${input} (${result.val} functions)`);
} else {
  if (!WebAssembly.validate(result.val)) {
    throw new Error("bootstrap compiler emitted invalid Wasm");
  }
  await writeFile(output, result.val);
  console.log(`Built ${output} (${result.val.length} bytes)`);
}
