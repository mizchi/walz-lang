import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  writeFile,
  readdir,
  copyFile,
  cp,
  rm,
} from "node:fs/promises";
import { resolve, join, dirname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { metrics } from "./source.mjs";

export const projectRoot = fileURLToPath(new URL("../../", import.meta.url));
export const evalRoot = fileURLToPath(new URL("../", import.meta.url));
const suite = JSON.parse(
  await readFile(join(evalRoot, "stock-ledger/suite.json"), "utf8"),
);
export const sha256 = (value) =>
  createHash("sha256").update(value).digest("hex");
export const json = async (path) => JSON.parse(await readFile(path, "utf8"));
export const save = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + "\n");
export const optional = async (path) => {
  try {
    return await json(path);
  } catch (e) {
    if (e.code === "ENOENT") return null;
    throw e;
  }
};
const quote = (text) => "'" + text.replaceAll("'", "'\\''") + "'";
export const command = (args) => args.map(quote).join(" ");

export async function files(directory) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await files(path)));
    else if (entry.isFile()) output.push(path);
  }
  return output.sort();
}
export async function implementationFingerprint(root = projectRoot) {
  const hashes = {};
  for (const name of [
    "src",
    "runtime",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "pnpm-lock.yaml",
    "evals/lib",
    "evals/stock-ledger",
    "evals/diagnostics",
    "evals/templates",
    "evals/run.mjs",
  ]) {
    const path = join(root, name);
    let paths;
    try {
      paths = await files(path);
    } catch (e) {
      if (e.code === "ENOTDIR") paths = [path];
      else if (e.code === "ENOENT") continue;
      else throw e;
    }
    for (const file of paths)
      if (/\.(rs|mjs|md|json|yaml)$|Cargo\.(toml|lock)$/.test(file))
        hashes[relative(root, file)] = sha256(await readFile(file));
  }
  return { sha256: sha256(JSON.stringify(hashes)), files: hashes };
}
export async function toolVersions() {
  const out = { node: process.version };
  for (const executable of ["cargo", "wasmtime"]) {
    try {
      out[executable] = (
        await promisify(execFile)(executable, ["--version"], {
          cwd: projectRoot,
          timeout: 5000,
        })
      ).stdout.trim();
    } catch {
      out[executable] = null;
    }
  }
  return out;
}
export function resolveRun(value) {
  return /^[\p{L}\p{N}][\p{L}\p{N}._-]*$/u.test(value)
    ? join(evalRoot, "runs", value)
    : resolve(value);
}
export async function prepareRun(
  id,
  {
    directory = join(evalRoot, "runs", id),
    root = projectRoot,
    suite: suiteName = "stock-ledger",
  } = {},
) {
  if (suiteName === "diagnostics") {
    const { prepareDiagnostics } = await import("../diagnostics/runs.mjs");
    return prepareDiagnostics(id, { directory, root });
  }
  if (suiteName !== "stock-ledger")
    throw new Error("unknown evaluation suite: " + suiteName);
  if (!/^[\p{L}\p{N}][\p{L}\p{N}._-]*$/u.test(id))
    throw new Error("run ID must be a name without path separators");
  directory = resolve(directory);
  await mkdir(dirname(directory), { recursive: true });
  await mkdir(directory); // Never overwrite a previous run or its answers.
  try {
    for (const role of ["reader", "writer"]) await mkdir(join(directory, role));
    for (const name of suite.support) {
      const destination = join(directory, "support", name);
      await mkdir(dirname(destination), { recursive: true });
      await cp(join(root, name), destination, { recursive: true });
    }
    for (const role of ["reader", "writer"]) {
      await copyFile(
        join(root, suite.cheatsheet),
        join(directory, role, "cheatsheet.md"),
      );
      await copyFile(
        join(root, suite.main),
        join(directory, role, "main.gwat"),
      );
      await copyFile(
        join(evalRoot, "stock-ledger/contract.md"),
        join(directory, role, "contract.md"),
      );
      await writeFile(
        join(directory, role, "program.gwat"),
        '(module (include "../support/examples/apps/stock-ledger/program.gwat"))\n',
      );
    }
    await copyFile(
      join(root, suite.counted),
      join(directory, "reader/counted.gwat"),
    );
    await copyFile(
      join(root, suite.main),
      join(directory, "writer/candidate.gwat"),
    );
    await copyFile(
      join(evalRoot, "stock-ledger/rubric.md"),
      join(directory, "reader/rubric.md"),
    );
    await copyFile(
      join(evalRoot, "stock-ledger/suite.json"),
      join(directory, "suite.json"),
    );
    await save(join(directory, "metrics.json"), {
      main: metrics(
        await readFile(join(directory, "reader/main.gwat"), "utf8"),
      ),
      counted: metrics(
        await readFile(join(directory, "reader/counted.gwat"), "utf8"),
      ),
    });
    const values = {
      READER_OUTPUT: join(directory, "reader.json"),
      CANDIDATE: join(directory, "writer/candidate.gwat"),
      WRITER_OUTPUT: join(directory, "writer/report.json"),
      PROJECT_ROOT: root,
      CHECK_COMMAND: command([
        "cargo",
        "run",
        "--locked",
        "--quiet",
        "--",
        "check",
        join(directory, "writer/candidate.gwat"),
      ]),
      REVIEW_COMMAND: command([
        "node",
        join(evalRoot, "run.mjs"),
        "review",
        directory,
        "--correct",
        "--reason",
        "型・制御・所有性の回答を配布コードの具体的な箇所に照合した理由",
      ]),
      VERIFY_COMMAND: command([
        "node",
        join(evalRoot, "run.mjs"),
        "verify",
        directory,
      ]),
      SCORE_COMMAND: command([
        "node",
        join(evalRoot, "run.mjs"),
        "score",
        directory,
      ]),
    };
    for (const role of ["reader", "writer", "reviewer"]) {
      let template = await readFile(
        join(evalRoot, "templates", role + ".md"),
        "utf8",
      );
      template = template.replace(
        /\{\{([A-Z_]+)\}\}/g,
        (_, name) => values[name],
      );
      await writeFile(
        join(
          directory,
          role === "reviewer" ? "reviewer-task.md" : role + "/task.md",
        ),
        template,
      );
    }
    const roles = {
      reader: [
        "reader/task.md",
        "reader/cheatsheet.md",
        "reader/contract.md",
        "reader/rubric.md",
        "reader/main.gwat",
        "reader/counted.gwat",
      ],
      writer: [
        "writer/task.md",
        "writer/cheatsheet.md",
        "writer/contract.md",
        "writer/main.gwat",
        "writer/candidate.gwat",
      ],
    };
    await save(join(directory, "manifest.json"), {
      schema: "gcwat-eval-v1",
      id,
      suite,
      preparedAt: new Date().toISOString(),
      projectRoot: root,
      versions: await toolVersions(),
      implementation: await implementationFingerprint(root),
      roles,
    });
    await writeFile(
      join(directory, "dispatch.md"),
      `# 評価役へ渡す指示\n\n各役は別の初見コンテキスト。会話履歴、過去の結果、他の役の出力を渡さない。\n\n- 読解役: ${join(directory, "reader/task.md")} を読み、その指示だけに従う。\n- 変更役: ${join(directory, "writer/task.md")} を読み、その指示だけに従う。\n\n親は両方が完了した後 reviewer-task.md を使う。このCLI自体はエージェントを生成しない。\n`,
    );
    const hashes = {};
    for (const file of await files(directory)) {
      const name = relative(directory, file);
      if (name !== "writer/candidate.gwat")
        hashes[name] = sha256(await readFile(file));
    }
    await save(join(directory, "inputs.json"), hashes);
    return directory;
  } catch (e) {
    await rm(directory, { recursive: true, force: true });
    throw e;
  }
}

export async function checkInputs(directory) {
  const hashes = await json(join(directory, "inputs.json"));
  if (!Object.keys(hashes).length) throw new Error("empty frozen input set");
  for (const [name, hash] of Object.entries(hashes)) {
    const path = resolve(directory, name);
    if (!path.startsWith(resolve(directory) + "/"))
      throw new Error("input outside run: " + name);
    if (sha256(await readFile(path)) !== hash)
      throw new Error("frozen input changed: " + name);
  }
  return sha256(JSON.stringify(hashes));
}
export async function readReader(directory) {
  if ((await json(join(directory, "manifest.json"))).suite.id === "diagnostics")
    return (await import("../diagnostics/runs.mjs")).readDiagnosticReader(
      directory,
    );
  const data = await json(join(directory, "reader.json"));
  const axes = (await json(join(directory, "manifest.json"))).suite.axes;
  for (const key of [
    "flow",
    "state",
    "alias",
    "ownership",
    "errors",
    "scope",
    "changes",
  ])
    if (typeof data.answers?.[key] !== "string" || !data.answers[key].trim())
      throw new Error("missing answer: " + key);
  if (
    Object.keys(data.scores ?? {})
      .sort()
      .join() !== [...axes].sort().join()
  )
    throw new Error("invalid score axes");
  for (const axis of axes) {
    const item = data.scores[axis];
    if (
      !Number.isInteger(item.score) ||
      item.score < 0 ||
      item.score > 4 ||
      typeof item.evidence !== "string" ||
      !item.evidence.trim()
    )
      throw new Error("invalid score or evidence: " + axis);
  }
  if (!Array.isArray(data.issues) || !Array.isArray(data.limitations))
    throw new Error("issues and limitations must be arrays");
  return data;
}
export async function recordReview(directory, { correct, reason }) {
  const inputFingerprint = await checkInputs(directory);
  await readReader(directory);
  if (
    typeof correct !== "boolean" ||
    typeof reason !== "string" ||
    !reason.trim()
  )
    throw new Error("review requires correct/incorrect and a reason");
  const result = {
    correct,
    reason,
    reviewedAt: new Date().toISOString(),
    inputsFingerprint: inputFingerprint,
    readerSha256: sha256(await readFile(join(directory, "reader.json"))),
  };
  await save(join(directory, "review.json"), result);
  return result;
}
export async function scoreRun(directory) {
  let result;
  try {
    const inputsFingerprint = await checkInputs(directory);
    const manifest = await json(join(directory, "manifest.json"));
    if (manifest.suite.id === "diagnostics")
      return (await import("../diagnostics/runs.mjs")).scoreDiagnostics(
        directory,
      );
    const data = await optional(join(directory, "reader.json"));
    const reader = data ? await readReader(directory) : null;
    const writer = await optional(join(directory, "writer/report.json"));
    if (
      writer &&
      (typeof writer.completed !== "boolean" ||
        ![
          "checks",
          "repairs",
          "ambiguities",
          "assumptions",
          "extraDocumentsRead",
        ].every((k) => Array.isArray(writer[k])))
    )
      throw new Error("invalid writer report");
    const scores = reader
      ? Object.fromEntries(
          manifest.suite.axes.map((k) => [k, reader.scores[k].score]),
        )
      : null;
    const review = await optional(join(directory, "review.json"));
    if (
      review &&
      (typeof review.correct !== "boolean" ||
        typeof review.reason !== "string" ||
        !review.reason.trim())
    )
      throw new Error("invalid review");
    const reviewed = !!(
      reader &&
      review &&
      review.inputsFingerprint === inputsFingerprint &&
      review.readerSha256 ===
        sha256(await readFile(join(directory, "reader.json")))
    );
    const execution = await optional(join(directory, "verification.json"));
    const implementation = await implementationFingerprint();
    const currentExecution = !!(
      execution &&
      execution.implementation?.sha256 === implementation.sha256 &&
      execution.node === process.version &&
      execution.inputsFingerprint === inputsFingerprint &&
      execution.candidateSha256 ===
        sha256(await readFile(join(directory, "writer/candidate.gwat"))) &&
      execution.sampleSha256 ===
        sha256(await readFile(join(directory, "reader/counted.gwat")))
    );
    const executionPassed = !!(
      currentExecution &&
      execution.passed === true &&
      ["sample", "candidate"].every((k) => {
        const checks = execution.subjects?.[k]?.checks;
        return (
          execution.subjects?.[k]?.passed === true &&
          checks?.length === manifest.suite.executionChecks &&
          checks.every((c) => c.passed === true)
        );
      })
    );
    const readable =
      !!scores &&
      Object.values(scores).every((v) => v >= manifest.suite.minimumPerAxis);
    const incomplete = !reader || !writer || !reviewed || !currentExecution;
    const writerPassed = !!(
      writer?.completed && writer.extraDocumentsRead.length === 0
    );
    const passed =
      !incomplete &&
      readable &&
      review.correct &&
      executionPassed &&
      writerPassed;
    result = {
      schema: "gcwat-eval-summary-v1",
      id: manifest.id,
      status: incomplete ? "incomplete" : passed ? "pass" : "fail",
      passed,
      inputsFingerprint,
      readability: {
        scores,
        total: scores ? Object.values(scores).reduce((a, b) => a + b, 0) : null,
        meetsTarget: readable,
      },
      answers: { reviewed, correct: reviewed ? review.correct : null },
      execution: {
        current: currentExecution,
        passed: executionPassed,
        implementation: execution?.implementation?.sha256 ?? null,
        implementationChangedSincePrepare:
          execution?.implementationChangedSincePrepare ?? null,
      },
      writer: {
        reported: !!writer,
        completed: writer?.completed ?? null,
        extraDocumentsRead: writer?.extraDocumentsRead ?? [],
      },
      notes: incomplete
        ? ["回答の監査または実行検証が未完了・更新前の結果です"]
        : [],
    };
  } catch (e) {
    result = {
      schema: "gcwat-eval-summary-v1",
      status: "invalid",
      passed: false,
      error: e.message,
    };
  }
  await save(join(directory, "summary.json"), result);
  return result;
}
export async function compareRuns(before, after) {
  const ma = await json(join(before, "manifest.json")),
    mb = await json(join(after, "manifest.json"));
  if (ma.suite.id === "diagnostics" && mb.suite.id === "diagnostics")
    return (await import("../diagnostics/runs.mjs")).compareDiagnostics(
      before,
      after,
    );
  const a = await scoreRun(before),
    b = await scoreRun(after);
  const ah = await json(join(before, "inputs.json")),
    bh = await json(join(after, "inputs.json"));
  const comparableRubric =
    ah["reader/rubric.md"] === bh["reader/rubric.md"] &&
    ah["suite.json"] === bh["suite.json"];
  return {
    before: a,
    after: b,
    comparableRubric,
    sourceMetrics: {
      before: await optional(join(before, "metrics.json")),
      after: await optional(join(after, "metrics.json")),
    },
    scoreDelta:
      comparableRubric && a.readability?.scores && b.readability?.scores
        ? Object.fromEntries(
            suite.axes.map((k) => [
              k,
              b.readability.scores[k] - a.readability.scores[k],
            ]),
          )
        : null,
    note: "点差は少数のモデルによる主観評価。人間の改善量や統計的な効果とは扱わない。",
  };
}
