import { createServer } from "node:http";
import { readFile, copyFile, mkdir, writeFile, rm } from "node:fs/promises";
import { resolve, join, extname } from "node:path";
import {buildNative} from "./build-native.mjs";
import {build as buildSuccinct} from './succinct/build.mjs';
import { build } from "./build.mjs";
import { build as buildCore } from "./build-core.mjs";
import {execFile as execFileCallback} from 'node:child_process';
import {promisify} from 'node:util';
const directory = resolve("target/browser-tests");
await mkdir(directory, { recursive: true });
await mkdir(join(directory,'workers'),{recursive:true});
for(const name of ['host.mjs','agent.mjs'])await copyFile(new URL('./workers/'+name,import.meta.url),join(directory,'workers',name));
for(const name of ['worker-driver.mjs','worker-wlt-driver.mjs'])await copyFile(new URL('./testing/'+name,import.meta.url),join(directory,'workers',name));
const wasiSource=join(directory,'wasi-source');await mkdir(wasiSource,{recursive:true});
for(const name of ['main.wlt','batch.wlt','filesystem.wit','policy.json'])await copyFile('examples/wasi-files/'+name,join(wasiSource,name));
await rm(join(wasiSource,'generated'),{recursive:true,force:true});
await promisify(execFileCallback)('cargo',['run','--locked','--quiet','--','bind',join(wasiSource,'filesystem.wit'),join(wasiSource,'policy.json'),join(wasiSource,'generated')]);
const privateThreads = join(directory, "threads-private.wlt");
await writeFile(privateThreads, (await readFile("tests/component-fixtures/threads.wlt", "utf8"))
  .replace('(import "env" "memory" (memory $shared 1 2 shared))', '(memory $shared 1 2 shared)'));
const privateCells = join(directory, "atomic-cells-private.wlt");
await writeFile(privateCells, (await readFile("tests/component-fixtures/atomic-cells.wlt", "utf8"))
  .replace('(import "env" "memory" (memory $shared 1 1 shared))', '(memory $shared 1 1 shared)'));
const jobs=[];
for(const backend of ['core','component'])jobs.push(()=>buildSuccinct(join(directory,backend,'succinct'),{backend}));
for (const [backend, compile] of [
  ["component", build],
  ["core", buildCore],
])
  for (const [fixture, source] of [
    ...["values", "event-loop", "buffers", "text-library", "module-imports", "json-value", "async-abi", "lists", "numeric-lists", "standard", "collections", "simd", "language-core", "struct-construction", "typeclasses", "views", "array-views"].map((name) => [
      name,
      `tests/component-fixtures/${name}.wlt`,
    ]),
    ["interface-types", "tests/component-fixtures/interface-types.wlt"],
    ...["wit-values","stream-batch","deadlines","local-cancellation","option-result","high-level","high-level-async","worker-job"].map(name=>[name,`tests/component-fixtures/${name}.wlt`]),
    ["wasm-bridge", "tests/component-fixtures/wasm-bridge.wlt"],
    ["scoped-tasks", "tests/component-fixtures/scoped-tasks.wlt"],
    ["channels", "examples/channels/main.wlt"],
    ["wasi-files", join(wasiSource,'main.wlt')],
    ["threads-private", privateThreads],
    ["atomic-cells-private", privateCells],
    ["simd-kernels", "examples/simd-kernels/portable.wlt"],
    ["resources", "examples/resources/main.wlt"],
    ["wit-bindings", "examples/wit/main.wlt"],
    ["text-io", "examples/io/text-filter.wlt"],
    ["log-stats", "examples/apps/log-stats/main.wlt"],
    ["stock-ledger", "examples/apps/stock-ledger/main.wlt"],
    ["workflow", "examples/apps/workflow/main.wlt"],
  ])
    jobs.push(()=>compile(source, join(directory, backend, fixture), fixture === "wit-bindings" ? {bindings:"examples/wit/generated/bindings.json"} : fixture === "resources" ? {bindings:"examples/resources/policy.json"} : fixture==='channels'?{bindings:'examples/channels/policy.json'}:fixture==='wasi-files'?{bindings:join(wasiSource,'generated/bindings.json')}:{}));
await copyFile('runtime/wasi-filesystem.mjs',join(directory,'wasi-filesystem.mjs'));
await copyFile('runtime/channels.mjs',join(directory,'channels.mjs'));
jobs.push(()=>buildCore("tests/component-fixtures/threads.wlt", join(directory, "core/threads")));
jobs.push(()=>buildCore("tests/component-fixtures/atomic-cells.wlt", join(directory, "core/atomic-cells")));
jobs.push(()=>buildNative("tests/component-fixtures/native-async.wlt","tests/component-fixtures/native-async.wit",join(directory,"native/async")));
jobs.push(()=>buildNative("tests/component-fixtures/native-resources.wlt","tests/component-fixtures/native-resources.wit",join(directory,"native/resources"),{bindings:"tests/component-fixtures/native-resources.json"}));
const count=jobs.length,started=performance.now();let completed=0;
await Promise.all(Array.from({length:2},async()=>{
  while(jobs.length){const compile=jobs.shift();await compile();completed++;
    if(completed%8===0||completed===count)console.error(`Browser fixtures: ${completed}/${count} (${Math.round((performance.now()-started)/1000)}s)`);
  }
}));
await copyFile(
  new URL("./text-io.mjs", import.meta.url),
  join(directory, "text-io.mjs"),
);
createServer(async (request, response) => {
  response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
  response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
  const pathname = new URL(request.url, "http://127.0.0.1").pathname;
  if (["/", "/component/", "/core/"].includes(pathname)) {
    response.writeHead(200, { "content-type": "text/html" });
    response.end("<!doctype html><title>wlt JSPI integration</title>");
    return;
  }
  const file = resolve(directory, `.${decodeURIComponent(pathname)}`);
  if (!file.startsWith(directory + "/")) {
    response.writeHead(403);
    response.end();
    return;
  }
  try {
    const bytes = await readFile(file);
    const type =
      extname(file) === ".wasm"
        ? "application/wasm"
        : [".js", ".mjs"].includes(extname(file))
          ? "text/javascript"
          : "application/octet-stream";
    response.writeHead(200, { "content-type": type });
    response.end(bytes);
  } catch {
    response.writeHead(404);
    response.end();
  }
}).listen(41739, "127.0.0.1");
