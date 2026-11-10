import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, copyFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  prepareRun,
  readReader,
  recordReview,
  scoreRun,
  compareRuns,
  checkInputs,
  implementationFingerprint,
  sha256,
} from "./lib/runs.mjs";
import { verifyRun } from "./lib/runner.mjs";
import { candidateHashes } from "./diagnostics/runs.mjs";

const json = async (p) => JSON.parse(await readFile(p, "utf8"));
const temp = async (run) => {
  const p = await mkdtemp(join(tmpdir(), "walz-diagnostic-eval-"));
  try {
    await run(p);
  } finally {
    await rm(p, { recursive: true, force: true });
  }
};
function answer(manifest, score = 3) {
  return {
    cases: Object.fromEntries(
      manifest.suite.cases.map((c) => [
        c.id,
        {
          cause: "コードと診断から特定した原因",
          fix: "意図を保った具体的な修正",
          explanation: "診断にある情報とコードから補った情報を区別した",
          scores: Object.fromEntries(
            manifest.suite.axes.map((a) => [
              a,
              { score, evidence: "program.walz の対象行と診断文を照合した" },
            ]),
          ),
        },
      ]),
    ),
    issues: [],
    limitations: [],
  };
}
async function fakeVerification(p) {
  const manifest = await json(join(p, "manifest.json"));
  await writeFile(
    join(p, "writer/report.json"),
    JSON.stringify({
      completed: true,
      checks: [],
      repairs: [],
      ambiguities: [],
      assumptions: [],
      extraDocumentsRead: [],
    }),
  );
  await writeFile(
    join(p, "verification.json"),
    JSON.stringify({
      passed: true,
      inputsFingerprint: await checkInputs(p),
      implementation: await implementationFingerprint(),
      node: process.version,
      candidateHashes: await candidateHashes(p),
      diagnosticsCurrent: true,
      cases: Object.fromEntries(
        manifest.suite.cases.map((c) => [
          c.id,
          {
            passed: true,
            original: { rejected: true, noPanic: true, locationPassed: true },
            control: { passed: true },
            repair: { passed: true },
          },
        ]),
      ),
    }),
  );
}

test(
  "diagnostic suite freezes real messages, rejects originals and checks meaningful controls/repairs",
  { timeout: 90000 },
  () =>
    temp(async (parent) => {
      const p = join(parent, "deep path's", "run");
      await prepareRun("errors-99", { directory: p, suite: "diagnostics" });
      const manifest = await json(join(p, "manifest.json"));
      assert.equal(manifest.suite.id, "diagnostics");
      assert.equal(manifest.suite.cases.length, 10);
      assert.ok(
        !manifest.roles.reader.some(
          (s) => s.includes("fixed") || s.includes("private"),
        ),
      );
      assert.ok(
        !manifest.roles.writer.some(
          (s) => s.includes("fixed") || s.includes("private"),
        ),
      );
      const packet = await json(
        join(p, "reader/cases/case-08/diagnostic.json"),
      );
      assert.equal(packet.exitCode, 1);
      assert.match(packet.stderr, /math.walz:\d+:\d+:/);
      const initial = await verifyRun(p, { timeoutMs: 60000 });
      assert.equal(initial.passed, false);
      assert.equal(initial.cases["case-08"].original.locationPassed, true);
      assert.equal(initial.cases["case-01"].original.locationPassed, true);
      assert.equal(initial.cases["case-09"].original.locationPassed, true);
      for (const c of Object.values(initial.cases)) {
        assert.equal(c.original.rejected, true, JSON.stringify(c));
        assert.equal(c.control.passed, true, JSON.stringify(c));
      }
      for (const c of manifest.suite.cases)
        for (const file of c.editable)
          await copyFile(
            join(p, "corpus", c.id, "fixed", file),
            join(p, "writer/cases", c.id, file),
          );
      const repaired = await verifyRun(p, { timeoutMs: 60000 });
      for (const c of Object.values(repaired.cases))
        assert.equal(c.repair.passed, true, JSON.stringify(c));
      await writeFile(
        join(p, "writer/cases/case-04/program.walz"),
        "(module (func $identity (param $value i32) (result i32) (local.get $value)))",
      );
      const erased = await verifyRun(p, { timeoutMs: 60000 });
      assert.equal(erased.cases["case-04"].repair.behaviorPassed, true);
      assert.equal(erased.cases["case-04"].repair.invariantsPassed, false);
      assert.equal(erased.passed, false);
      await writeFile(join(p, "reader/cases/case-01/diagnostic.json"), "{}");
      await assert.rejects(checkInputs(p), /diagnostic.json/);
    }),
);

test(
  "quality is per-case, audited, and invalidated by changed repairs or stale diagnostics",
  { timeout: 60000 },
  () =>
    temp(async (parent) => {
      const p = join(parent, "run");
      await prepareRun("quality", { directory: p, suite: "diagnostics" });
      const manifest = await json(join(p, "manifest.json")),
        data = answer(manifest, 4);
      await writeFile(join(p, "reader.json"), JSON.stringify(data));
      assert.equal((await scoreRun(p)).status, "incomplete");
      await fakeVerification(p);
      await recordReview(p, {
        correct: true,
        reason: "全ケースの原因・修正・診断中の情報を監査",
      });
      assert.equal((await scoreRun(p)).status, "pass");
      data.cases[manifest.suite.cases[0].id].scores.repair.score = 2;
      await writeFile(join(p, "reader.json"), JSON.stringify(data));
      await recordReview(p, {
        correct: true,
        reason: "弱い1軸を含めて説明と根拠は正しい",
      });
      assert.equal((await scoreRun(p)).status, "fail");
      data.cases[manifest.suite.cases[0].id].scores.repair.score = "3";
      await writeFile(join(p, "reader.json"), JSON.stringify(data));
      await assert.rejects(readReader(p), /score/);
      await writeFile(join(p, "reader.json"), JSON.stringify(answer(manifest)));
      await recordReview(p, { correct: true, reason: "修正した回答を再監査" });
      const verification = await json(join(p, "verification.json"));
      verification.diagnosticsCurrent = false;
      await writeFile(
        join(p, "verification.json"),
        JSON.stringify(verification),
      );
      assert.equal((await scoreRun(p)).status, "incomplete");
      await fakeVerification(p);
      await writeFile(join(p, "writer/cases/case-01/program.walz"), "changed");
      assert.equal((await scoreRun(p)).status, "incomplete");
    }),
);

test(
  "diagnostic comparisons require the same corpus as well as the same rubric",
  { timeout: 60000 },
  () =>
    temp(async (parent) => {
      const a = join(parent, "a"),
        b = join(parent, "b");
      for (const [id, p] of [
        ["before", a],
        ["after", b],
      ]) {
        await prepareRun(id, { directory: p, suite: "diagnostics" });
        const manifest = await json(join(p, "manifest.json"));
        await writeFile(
          join(p, "reader.json"),
          JSON.stringify(answer(manifest, id === "before" ? 3 : 4)),
        );
      }
      let comparison = await compareRuns(a, b);
      assert.equal(comparison.comparableRubric, true);
      assert.equal(comparison.scoreDelta["case-01"].cause, 1);
      // A different recorded corpus revision must not be compared on points.
      const manifest = await json(join(b, "manifest.json"));
      manifest.corpusFingerprint = "other revision";
      await writeFile(join(b, "manifest.json"), JSON.stringify(manifest));
      const inputs = await json(join(b, "inputs.json"));
      inputs["manifest.json"] = sha256(
        await readFile(join(b, "manifest.json")),
      );
      await writeFile(join(b, "inputs.json"), JSON.stringify(inputs));
      comparison = await compareRuns(a, b);
      assert.equal(comparison.comparableRubric, false);
      assert.equal(comparison.scoreDelta, null);
    }),
);
