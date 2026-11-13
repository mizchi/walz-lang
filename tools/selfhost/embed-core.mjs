// Rebuild the immutable bootstrap-library resource; this tool is never used at runtime.
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
export function renderBootstrapCore(source) {
  const chunks = [];
  for (let offset = 0; offset < source.length; offset += 4096) {
    let text = "";
    for (const byte of source.subarray(offset, offset + 4096))
      text +=
        byte >= 32 && byte <= 126 && byte !== 34 && byte !== 92
          ? String.fromCharCode(byte)
          : "\\" + byte.toString(16).padStart(2, "0");
    chunks.push(`(@string.bytes "${text}")`);
  }
  let body = chunks.shift() ?? "(@bytes.new)";
  for (const chunk of chunks) body = `(call $bytes.concat ${body} ${chunk})`;
  return (
    ";; Generated from bootstrap-core.sexp; run just selfhost-core-source.\n" +
    '(module (@use "walz:core" (type $bytes)) (@use "../core/bytes.walz" (func $bytes.concat)) (@export (func $core.source))\n' +
    `  (func $core.source (result $bytes) (@effect reads writes allocates) ${body}))\n`
  );
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const input = new URL(
      "../../walz/frontend/bootstrap-core.sexp",
      import.meta.url,
    ),
    output = new URL("../../walz/frontend/core-source.walz", import.meta.url);
  const expected = renderBootstrapCore(await readFile(input));
  if (process.argv.includes("--check")) {
    if ((await readFile(output, "utf8")) !== expected)
      throw new Error(
        "bootstrap core resource is stale; run just selfhost-core-source",
      );
  } else await writeFile(output, expected);
}
