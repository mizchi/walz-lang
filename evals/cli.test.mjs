import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
const exec = promisify(rawExec),
  cli = fileURLToPath(new URL("run.mjs", import.meta.url));
test("CLI works outside project, reports incomplete as exit 2, and rejects missing/unknown options", async () => {
  const parent = await mkdtemp(join(tmpdir(), "walz-eval-cli-"));
  try {
    const directory = join(parent, "run");
    const out = await exec(
      process.execPath,
      [cli, "prepare", "cli-99", "--output", directory],
      { cwd: parent },
    );
    assert.ok(out.stdout.includes(directory));
    assert.match(
      await readFile(join(directory, "dispatch.md"), "utf8"),
      /reader\/task.md/,
    );
    await assert.rejects(
      exec(process.execPath, [cli, "score", directory], { cwd: parent }),
      (error) => error.code === 2,
    );
    for (const args of [
      ["prepare", "bad", "--output"],
      ["prepare", "bad", "--typo"],
      ["score", directory, "--typo"],
    ])
      await assert.rejects(
        exec(process.execPath, [cli, ...args], { cwd: parent }),
        (error) => error.code === 1 && /option/.test(error.stderr),
      );
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
});
