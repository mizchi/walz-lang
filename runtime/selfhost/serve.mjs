import { createServer } from "node:http";
import { readFile, copyFile } from "node:fs/promises";
import { join, resolve, extname } from "node:path";
import { build as core } from "../build-core.mjs";
import { build as component } from "../build.mjs";
const root = resolve(process.env.WALZ_SELFHOST_TEST_ROOT ?? "target/selfhost-browser");
for (const [name, build] of [
  ["core", core],
  ["component", component],
])
  await build(
    "tests/component-fixtures/selfhost.walz",
    join(root, name, "selfhost"),
  );
await copyFile(
  new URL("../selfhost-gc/corpus.mjs", import.meta.url),
  join(root, "gc-corpus.mjs"),
);
await copyFile(
  new URL("../../examples/selfhost/gc.walz", import.meta.url),
  join(root, "gc-example.walz"),
);
await copyFile(
  new URL("../selfhost-front/corpus.mjs", import.meta.url),
  join(root, "front-corpus.mjs"),
);
await copyFile(
  new URL("../selfhost-front/macros.mjs", import.meta.url),
  join(root, "macros.mjs"),
);
await copyFile(
  new URL("../selfhost-front/connections.mjs", import.meta.url),
  join(root, "connections.mjs"),
);
createServer(async (request, response) => {
  try {
    const path = resolve(
      root,
      "." + new URL(request.url, "http://localhost").pathname,
    );
    if (!path.startsWith(root + "/") && path !== root) {
      response.writeHead(403).end();
      return;
    }
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    const ext = extname(path);
    response.setHeader(
      "Content-Type",
      ext === ".wasm"
        ? "application/wasm"
        : ext === ".js" || ext === ".mjs"
          ? "text/javascript"
          : "text/html",
    );
    response.end(
      ext
        ? await readFile(path)
        : "<!doctype html><title>Selfhost checks</title>",
    );
  } catch {
    response.writeHead(404).end();
  }
}).listen(Number(process.env.WALZ_SELFHOST_TEST_PORT ?? 41741), "127.0.0.1");
