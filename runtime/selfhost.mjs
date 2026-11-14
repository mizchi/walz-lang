// File I/O and bootstrap module loading only. Parsing/checking/encoding run in Walz.
import { readFile, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { readProject } from "./selfhost/files.mjs";

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
  const messages = {
    1: "invalid source UTF-8",
    2: "unexpected token byte",
    3: "missing parenthesis or expression",
    4: "unterminated block comment",
    5: "invalid string or escape",
    7: "source size limit",
    8: "syntax node limit",
    9: "syntax nesting limit",
    100: "expected one module",
    101: "unsupported source form",
    102: "duplicate function",
    103: "unknown function",
    104: "invalid signature or duplicate local name",
    105: "invalid integer constant",
    106: "wrong operand or call arity",
    107: "duplicate export name",
    108: "invalid UTF-8 export name",
    109: "unknown local",
    110: "type or stack-height mismatch",
    111: "operand stack underflow",
    112: "unknown branch label",
    113: "cannot assign to an immutable let binding",
    114: "invalid IR instruction",
    115: "invalid control structure",
    116: "ambiguous scalar type; add an annotation",
    117: "invalid source range",
    118: "invalid or unsupported value/GC type, declaration or subtype",
    119: "unknown or incompatible GC type/field",
    120: "cannot write an immutable GC field",
    121: "GC constructor requires defaultable fields",
    122: "packed field requires a matching signed/unsigned getter",
    123: "mutable GC effects exceed the declared effect bound",
    150: "compiler resource limit",
    200: "invalid or missing relative module path",
    201: "unknown, private, duplicate, or incompatible module symbol",
    202: "opaque representation access or unchecked cast across an opaque boundary",
    203: "cyclic module dependencies",
    205: "invalid loop, recur arity, or non-tail recur",
    206: "invalid or conflicting type arguments",
    207: "type argument constraint is not satisfied or not supported",
    208: "invalid or non-exhaustive pattern",
    209: "string literal contains invalid UTF-8",
    210: "malformed generic application",
    211: "invalid macro definition or clause",
    212: "unknown macro",
    213: "no macro rule matches the arguments and type guards",
    214: "invalid macro template or repetition lengths",
    215: "unbound definition-site name in macro template",
    216: "invalid macro binding scope or delegated interface",
    217: "macro expansion limit",
    218: "invalid macro type projection or missing expected type",
    219: "@bind has been removed; use @connect for Component connections",
    220: "binary @use requires (component $Constructor) and explicit instantiate; use the main compiler for Component output",
    221: "Component composition requires the main compiler; the bootstrap backend emits Core Wasm",
    222: "@lib has no executable entry; select its API from a module entry",
    224: "invalid named struct fields: duplicate, missing, unnamed or unknown field",
    225: "invalid, unsupported, recursive or non-portable @abi.type view",
    226: "ABI view exceeds implementation limits (flat slots, layout, shape or depth)",
    223: "@lib cannot declare Core exports, start, global or data initialization; publish source names with @export",
  };
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
