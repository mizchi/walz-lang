import { transpileBytes } from "@bytecodealliance/jco-transpile";
import { execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";
import { mkdir, readFile, writeFile, copyFile } from "node:fs/promises";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { adaptJspiGlue } from "./jco-compat.mjs";

const execFile = promisify(execFileCallback);
const root = fileURLToPath(new URL("..", import.meta.url));

export async function build(source, directory, options = {}) {
  const output = resolve(directory);
  await mkdir(output, { recursive: true });
  const binary = join(output, "component.wasm");
  await execFile(
    "cargo",
    ["run", "--locked", "--quiet", "--", ...(options.legacySyntax ? ["--legacy-syntax"] : []), "component", resolve(source), binary, ...(options.bindings ? ["--bindings", resolve(options.bindings)] : [])],
    { cwd: root, maxBuffer: 4 * 1024 * 1024 },
  );
  const manifest = JSON.parse(await readFile(`${binary}.json`, "utf8"));
  if (manifest.version !== 2 || !Array.isArray(manifest.asyncExports))
    throw new Error("unsupported component manifest version");
  const generated = await transpileBytes(await readFile(binary), {
    name: "component",
    instantiation: "async",
    asyncMode: "jspi",
    wasiShim: false,
    nodejsCompat: false,
    base64Cutoff: 0,
    noComponentErrorWrapping: false,
    bindgenEnableWasmExnref: true,
  });
  for (const [name, bytes] of Object.entries(generated.files)) {
    const path = resolve(output, name);
    if (!path.startsWith(output + "/"))
      throw new Error("invalid generated artifact path");
    await mkdir(dirname(path), { recursive: true });
    await writeFile(
      path,
      name.endsWith(".js")
        ? adaptJspiGlue(new TextDecoder().decode(bytes))
        : bytes,
    );
  }
  await copyFile(
    new URL("./host.mjs", import.meta.url),
    join(output, "host.mjs"),
  );
  await copyFile(
    new URL("./tasks.mjs", import.meta.url),
    join(output, "tasks.mjs"),
  );
  await copyFile(new URL("./resources.mjs",import.meta.url),join(output,"resources.mjs"));
  await writeFile(
    join(output, "package.json"),
    '{"type":"module","private":true}\n',
  );
  await writeFile(
    join(output, "index.js"),
    `import {instantiate as create} from './component.js';
import {createHostImports,createDelivery} from './host.mjs';
import {Tasks} from './tasks.mjs';
export {TaskCancelledError} from './tasks.mjs';
const factories=${JSON.stringify(manifest.factories)};
const asyncExports=${JSON.stringify(manifest.asyncExports)}.map(name=>name.replace(/-([a-z0-9])/g,(_,c)=>c.toUpperCase()));
export async function instantiate(options={}) {
  if (typeof WebAssembly.Suspending!=='function' || typeof WebAssembly.promising!=='function') throw new Error('WebAssembly JSPI is required');
  const tasks=new Tasks({...options,asyncExports});
  const delivery=createDelivery(factories);
  const imports={
    ...createHostImports(options,tasks,${JSON.stringify(manifest.imports ?? [])}),
    'wlt:runtime/delivery':delivery,'wlt:runtime/delivery@0.1.0':delivery,
  };
  let getCoreModule=options.getCoreModule;
  if (!getCoreModule && typeof process!=='undefined' && process.versions?.node) {
    getCoreModule=async name=>WebAssembly.compile(await (await import('node:fs/promises')).readFile(new URL(name,import.meta.url)));
  }
  const instantiateCore=async(module,imports)=>tasks.capture(await (options.instantiateCore??WebAssembly.instantiate)(module,imports));
  return tasks.bind(await create(getCoreModule,imports,instantiateCore,{wlt:{read:tasks.read.bind(tasks),disposal:tasks.disposal.bind(tasks),cleaning:()=>tasks.cleaning===true}}));
}
`,
  );
  return output;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const [source, directory] = process.argv.slice(2);
  if (!source || !directory)
    throw new Error(
      "usage: node runtime/build.mjs source.wlt output-directory",
    );
  await build(source, directory);
}
