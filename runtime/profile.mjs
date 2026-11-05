import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import { mkdir, readFile, writeFile, copyFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { cpus, release } from "node:os";
import { build } from "./build-core.mjs";
import { summarize } from "./profile-summary.mjs";
const exec = promisify(rawExec);
const root = fileURLToPath(new URL("..", import.meta.url));
const source = join(root, "examples/io/text-filter.wlt");
const productionFiles = [
  "core.wasm",
  "memory.wasm",
  "index.js",
  "bindings.mjs",
  "core.mjs",
  "host.mjs",
  "tasks.mjs",
];
export async function artifactHash(directory) {
  const hash = createHash("sha256");
  for (const name of productionFiles)
    hash.update(
      name +
        ":" +
        createHash("sha256")
          .update(await readFile(join(directory, name)))
          .digest("hex") +
        "\n",
    );
  return hash.digest("hex");
}
export async function capture(directory, rounds = 40) {
  if (!Number.isInteger(rounds) || rounds < 1 || rounds > 1000)
    throw new Error("rounds must be an integer from 1 to 1000");
  const output = resolve(directory);
  await mkdir(output, { recursive: true });
  await build(source, output);
  await exec(
    "cargo",
    ["run", "--locked", "--quiet", "--", "profile-text", source, output],
    { cwd: root },
  );
  await copyFile(source, join(output, "source.wlt"));
  const args = [
    "run",
    "-W",
    "gc=y,exceptions=y,function-references=y",
    `--profile=guest,${join(output, "guest.json")},1ms`,
    "--preload",
    `__component=${join(output, "env.wasm")}`,
    "--preload",
    `guest=${join(output, "core.wasm")}`,
    "--invoke",
    "run",
    join(output, "driver.wasm"),
    "30000",
    "64",
    String(rounds),
    "0",
  ];
  const native = await exec("wasmtime", args, {
    timeout: 60000,
    maxBuffer: 4 * 1024 * 1024,
  });
  const [written, allocations, requestedBytes, peakLive, linearBytes, live] =
    native.stdout.trim().split(/\s+/).map(Number);
  if (written !== 26250 * rounds || live !== 0)
    throw new Error("profile driver result or cleanup mismatch");
  const profile = summarize(
    JSON.parse(await readFile(join(output, "guest.json"), "utf8")),
  );
  if (profile.samples < 50)
    throw new Error("too few guest samples; increase the workload");
  const node = await exec(
    process.execPath,
    [fileURLToPath(new URL("./profile-node.mjs", import.meta.url)), output],
    { cwd: root, timeout: 120000 },
  );
  const report = {
    productionHash: await artifactHash(output),
    host: {
      platform: process.platform,
      arch: process.arch,
      release: release(),
      cpu: cpus()[0]?.model,
    },
    date: new Date().toISOString(),
    wasmtime: (await exec("wasmtime", ["--version"])).stdout.trim(),
    guestHash: createHash("sha256")
      .update(await readFile(join(output, "core.wasm")))
      .digest("hex"),
    native: {
      rows: 30000 * rounds,
      width: 64,
      written,
      allocations,
      requestedBytes,
      peakLive,
      linearBytes,
      live,
    },
    profile,
    node: JSON.parse(await readFile(join(output, "node-metrics.json"), "utf8")),
  };
  await writeFile(
    join(output, "report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(
    JSON.stringify({
      directory: output,
      native: report.native,
      top: profile.self.slice(0, 8),
    }),
  );
  console.log(node.stdout.trim());
  return report;
}
export async function compare(before, after) {
  const [a, b] = await Promise.all(
    [before, after].map((dir) =>
      readFile(join(resolve(dir), "report.json"), "utf8").then(JSON.parse),
    ),
  );
  const hashes = await Promise.all(
    [before, after].map((dir) => artifactHash(resolve(dir))),
  );
  for (const [i, report] of [a, b].entries())
    if (report.productionHash && report.productionHash !== hashes[i])
      throw new Error("profiling artifacts changed after capture");
  if (hashes[0] === hashes[1])
    throw new Error(
      "production artifacts are identical; no tuning change to compare",
    );
  const result = await exec(
    process.execPath,
    [
      fileURLToPath(new URL("./profile-node.mjs", import.meta.url)),
      resolve(before),
      resolve(after),
    ],
    { cwd: root, timeout: 120000, maxBuffer: 4 * 1024 * 1024 },
  );
  const report = JSON.parse(result.stdout);
  await writeFile(
    join(resolve(after), "comparison.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  return report;
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const [command, a, b] = process.argv.slice(2);
  if (command === "capture" && a)
    await capture(a, b === undefined ? 40 : Number(b));
  else if (command === "compare" && a && b)
    console.log(JSON.stringify(await compare(a, b), null, 2));
  else
    throw new Error(
      "usage: profile.mjs capture directory [native-rounds] | compare before after",
    );
}
