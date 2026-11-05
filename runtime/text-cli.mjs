import { createReadStream, createWriteStream } from "node:fs";
import { stat } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { runTextProgram } from "./node-io.mjs";

export async function main(args) {
  const [directory, ...flags] = args;
  if (!directory)
    throw new Error(
      "usage: text-cli.mjs output-directory [--input file] [--output file]",
    );
  let input = process.stdin,
    output = process.stdout,
    inputPath,
    outputPath;
  for (let i = 0; i < flags.length; i += 2) {
    const [flag, path] = flags.slice(i, i + 2);
    if (!path || !["--input", "--output"].includes(flag))
      throw new Error("expected --input file or --output file");
    if (flag === "--input") {
      inputPath = path;
      input = () => createReadStream(path);
    } else {
      outputPath = path;
      output = () => createWriteStream(path);
    }
  }
  if (inputPath && outputPath) {
    const [a, b] = await Promise.all([
      stat(inputPath).catch(() => null),
      stat(outputPath).catch(() => null),
    ]);
    if (
      resolve(inputPath) === resolve(outputPath) ||
      (a && b && a.dev === b.dev && a.ino === b.ino)
    )
      throw new Error("input and output must differ");
  }
  const controller = new AbortController();
  const interrupt = () => controller.abort("SIGINT");
  process.on("SIGINT", interrupt);
  try {
    const result = await runTextProgram(directory, {
      input,
      output,
      signal: controller.signal,
    });
    if (result?.tag === "ok") return 0;
    if (result?.tag === "err") {
      console.error(`program error ${result.val}`);
      return 1;
    }
    throw new Error("main must return result<i32,i32>");
  } catch (error) {
    if (error?.name === "TaskCancelledError") return 130;
    throw error;
  } finally {
    process.off("SIGINT", interrupt);
  }
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
