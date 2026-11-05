import { createServer } from "node:http";
import { readFile, copyFile } from "node:fs/promises";
import { resolve, join, extname } from "node:path";
import { build } from "./build.mjs";
import { build as buildCore } from "./build-core.mjs";
const directory = resolve("target/browser-tests");
for (const [backend, compile] of [
  ["component", build],
  ["core", buildCore],
])
  for (const [fixture, source] of [
    ...["values", "event-loop", "buffers", "text-library", "module-imports", "json-value", "async-abi"].map((name) => [
      name,
      `tests/component-fixtures/${name}.wlt`,
    ]),
    ["resources", "examples/resources/main.wlt"],
    ["wit-bindings", "examples/wit/main.wlt"],
    ["text-io", "examples/io/text-filter.wlt"],
    ["log-stats", "examples/apps/log-stats/main.wlt"],
    ["stock-ledger", "examples/apps/stock-ledger/main.wlt"],
    ["workflow", "examples/apps/workflow/main.wlt"],
  ])
    await compile(source, join(directory, backend, fixture), fixture === "wit-bindings" ? {bindings:"examples/wit/generated/bindings.json"} : fixture === "resources" ? {bindings:"examples/resources/policy.json"} : {});
await copyFile(
  new URL("./text-io.mjs", import.meta.url),
  join(directory, "text-io.mjs"),
);
createServer(async (request, response) => {
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
