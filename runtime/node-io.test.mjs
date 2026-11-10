import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn, execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";
import { once } from "node:events";
import { build } from "./build-core.mjs";
const execFile = promisify(execFileCallback);
const cli = resolve("runtime/text-cli.mjs");

test("text CLI preserves application error codes without labelling them as I/O failures", async () => {
  const dir = await mkdtemp(join(tmpdir(), "walz-app-error-"));
  try {
    const source = join(dir, "error.walz");
    await writeFile(
      source,
      "(module (func $main (export \"main\") (result (@result i32 i32)) (@effect pure) (@result.err i32 i32 (i32.const 1001))))",
    );
    await build(source, join(dir, "app"));
    await assert.rejects(
      execFile(process.execPath, [cli, join(dir, "app")]),
      (error) =>
        error.code === 1 && error.stderr.trim() === "program error 1001",
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test(
  "text CLI filters UTF-8 from pipes and files, reports I/O errors and acknowledges SIGINT",
  { timeout: 60000 },
  async () => {
    const dir = await mkdtemp(join(tmpdir(), "walz-cli-"));
    try {
      const app = join(dir, "app");
      await build("examples/io/text-filter.walz", app);
      const input = join(dir, "input.txt"),
        output = join(dir, "output.txt");
      await writeFile(input, "日本語🦀\r\n\nend");
      const result = await execFile(process.execPath, [
        cli,
        app,
        "--input",
        input,
        "--output",
        output,
      ]);
      assert.equal(result.stdout, "");
      assert.equal(await readFile(output, "utf8"), "日本語🦀\nend\n");
      const emptyInput = join(dir, "empty.txt");
      for (const text of ["", "\n\r\n"]) {
        await writeFile(emptyInput, text);
        await writeFile(output, "stale output");
        await execFile(process.execPath, [
          cli,
          app,
          "--input",
          emptyInput,
          "--output",
          output,
        ]);
        assert.equal(await readFile(output, "utf8"), "");
      }

      await assert.rejects(
        execFile(process.execPath, [
          cli,
          app,
          "--input",
          input,
          "--output",
          input,
        ]),
        (error) =>
          error.code === 1 &&
          error.stderr.includes("input and output must differ"),
      );
      assert.equal(await readFile(input, "utf8"), "日本語🦀\r\n\nend");
      await assert.rejects(
        execFile(process.execPath, [
          cli,
          app,
          "--input",
          input,
          "--output",
          dir,
        ]),
        (error) => error.code === 1 && error.stderr.includes("program error 3"),
      );

      await assert.rejects(
        execFile(process.execPath, [cli, app, "--input", join(dir, "absent")]),
        (error) => error.code === 1 && error.stderr.includes("program error 2"),
      );
      const child = spawn(process.execPath, [cli, app], {
        stdio: ["pipe", "pipe", "pipe"],
      });
      let text = "",
        errors = "";
      child.stdout.setEncoding("utf8");
      child.stderr.setEncoding("utf8");
      child.stdout.on("data", (data) => (text += data));
      child.stderr.on("data", (data) => (errors += data));
      const closed = once(child, "close");
      try {
        const first = once(child.stdout, "data");
        child.stdin.write("first\n");
        await first;
        child.kill("SIGINT");
        assert.deepEqual(await closed, [130, null]);
        assert.equal(text, "first\n");
        assert.equal(errors, "");
      } finally {
        if (child.exitCode === null) child.kill("SIGKILL");
      }
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  },
);
