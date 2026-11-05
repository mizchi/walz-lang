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
    ...["values", "event-loop", "buffers", "text-library"].map((name) => [
      name,
      `tests/component-fixtures/${name}.gwat`,
    ]),
    ["text-io", "examples/io/text-filter.gwat"],
    ["log-stats", "examples/apps/log-stats/main.gwat"],
    ["stock-ledger", "examples/apps/stock-ledger/main.gwat"],
    ["workflow", "examples/apps/workflow/main.gwat"],
  ])
    await compile(source, join(directory, backend, fixture));
await copyFile(
  new URL("./text-io.mjs", import.meta.url),
  join(directory, "text-io.mjs"),
);
createServer(async (request, response) => {
  const pathname = new URL(request.url, "http://127.0.0.1").pathname;
  if (["/", "/component/", "/core/"].includes(pathname)) {
    response.writeHead(200, { "content-type": "text/html" });
    response.end("<!doctype html><title>gcwat JSPI integration</title>");
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
