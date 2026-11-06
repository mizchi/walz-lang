import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, rm, access, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  prepareRun,
  readReader,
  checkInputs,
  recordReview,
  scoreRun,
  compareRuns,
  sha256,
  implementationFingerprint,
} from "./lib/runs.mjs";

const reader = (score = 3) => ({
  answers: Object.fromEntries(
    ["flow", "state", "alias", "ownership", "errors", "scope", "changes", "syntax", "bindings", "standard", "api", "counter"].map(
      (k) => [k, "コードと契約に基づく具体的な回答"],
    ),
  ),
  scores: Object.fromEntries(
    ["flow", "types", "scope", "layout", "changes"].map((k) => [
      k,
      { score, evidence: "counted.wlt:5 の契約と7行目の束縛を対応させた" },
    ]),
  ),
  issues: [],
  limitations: [],
});
const temporary = async (run) => {
  const parent = await mkdtemp(join(tmpdir(), "wlt-eval-"));
  try {
    await run(parent);
  } finally {
    await rm(parent, { recursive: true, force: true });
  }
};
async function fakeVerification(directory) {
  await writeFile(
    join(directory, "writer/report.json"),
    JSON.stringify({
      completed: true,
      checks: [],
      repairs: [],
      ambiguities: [],
      assumptions: [],
      extraDocumentsRead: [],
    }),
  );
  const manifest = JSON.parse(
    await readFile(join(directory, "manifest.json"), "utf8"),
  );
  const hashes = JSON.parse(
    await readFile(join(directory, "inputs.json"), "utf8"),
  );
  await writeFile(
    join(directory, "verification.json"),
    JSON.stringify({
      passed: true,
      implementation: await implementationFingerprint(),
      node: process.version,
      inputsFingerprint: sha256(JSON.stringify(hashes)),
      candidateSha256: sha256(
        await readFile(join(directory, "writer/candidate.wlt")),
      ),
      sampleSha256: sha256(
        await readFile(join(directory, "reader/counted.wlt")),
      ),
      subjects: Object.fromEntries(
        ["sample", "candidate"].map((k) => [
          k,
          {
            passed: true,
            checks: Array.from(
              { length: manifest.suite.executionChecks + (k === "candidate" ? (manifest.suite.additionalCandidateChecks ?? 0) : 0) },
              (_, i) => ({ name: String(i), passed: true }),
            ),
          },
        ]),
      ),
    }),
  );
}

test("prepare works outside the checkout, freezes dependencies, separates roles and refuses overwrite", () =>
  temporary(async (parent) => {
    const directory = join(parent, "deep path's", "run-99");
    await prepareRun("round-99", { directory });
    const manifest = JSON.parse(
      await readFile(join(directory, "manifest.json"), "utf8"),
    );
    assert.equal(manifest.id, "round-99");
    assert.ok(manifest.roles.reader.includes("reader/counted.wlt"));
    assert.ok(manifest.roles.reader.includes("reader/api.md"));
    assert.ok(manifest.roles.writer.includes("writer/api.md"));
    const api = await readFile(join(directory, "writer/api.md"), "utf8");
    assert.match(api, /\(func \$string.parse-u64/);
    assert.match(api, /具体化時に推論/);
    assert.ok(!api.includes("(func $bytes.matches"));
    assert.ok(!manifest.roles.writer.some((p) => p.includes("counted")));
    await assert.rejects(access(join(directory, "writer/counted.wlt")));
    const bridge = await readFile(
      join(directory, "writer/program.wlt"),
      "utf8",
    );
    assert.equal(
      bridge,
      '(module (%include "../support/examples/apps/stock-ledger/program.wlt"))\n',
    );
    await checkInputs(directory);
    await writeFile(join(directory, "reader.json"), "keep");
    await assert.rejects(prepareRun("round-99", { directory }), /exist/i);
    assert.equal(
      await readFile(join(directory, "reader.json"), "utf8"),
      "keep",
    );
    await writeFile(join(directory, "support/stdlib/text.wlt"), "changed");
    await assert.rejects(checkInputs(directory), /text.wlt/);
  }));

test("score needs audited answers and both executions; it rejects stale review and stale candidate results", () =>
  temporary(async (parent) => {
    await prepareRun("review", { directory: parent + "/run" });
    const directory = parent + "/run";
    await writeFile(join(directory, "reader.json"), JSON.stringify(reader()));
    assert.equal((await scoreRun(directory)).status, "incomplete");
    await fakeVerification(directory);
    assert.equal((await scoreRun(directory)).status, "incomplete");
    await recordReview(directory, {
      correct: true,
      reason: "型・所有性・終了処理の回答をソースに照合した",
    });
    assert.equal((await scoreRun(directory)).status, "pass");
    const execution = JSON.parse(
      await readFile(join(directory, "verification.json"), "utf8"),
    );
    execution.implementation.sha256 = "old implementation";
    await writeFile(
      join(directory, "verification.json"),
      JSON.stringify(execution),
    );
    assert.equal((await scoreRun(directory)).status, "incomplete");
    await fakeVerification(directory);
    const changed = reader();
    changed.answers.flow = "更新した回答";
    await writeFile(join(directory, "reader.json"), JSON.stringify(changed));
    assert.equal((await scoreRun(directory)).status, "incomplete");
    await recordReview(directory, {
      correct: true,
      reason: "更新した回答も再照合した",
    });
    await writeFile(join(directory, "writer/candidate.wlt"), "changed");
    assert.equal((await scoreRun(directory)).status, "incomplete");
  }));

test("one weak readability axis or incorrect answers cannot be hidden by a high total", () =>
  temporary(async (parent) => {
    const directory = parent + "/run";
    await prepareRun("weak", { directory });
    const data = reader(4);
    data.scores.scope.score = 2;
    await writeFile(join(directory, "reader.json"), JSON.stringify(data));
    await fakeVerification(directory);
    await recordReview(directory, { correct: true, reason: "回答を確認" });
    const result = await scoreRun(directory);
    assert.equal(result.readability.total, 18);
    assert.equal(result.status, "fail");
    await writeFile(join(directory, "reader.json"), JSON.stringify(reader(4)));
    await recordReview(directory, {
      correct: false,
      reason: "EOF後のcloseを不要と誤読している",
    });
    assert.equal((await scoreRun(directory)).status, "fail");
    const review = JSON.parse(
      await readFile(join(directory, "review.json"), "utf8"),
    );
    review.correct = "false";
    await writeFile(join(directory, "review.json"), JSON.stringify(review));
    assert.equal((await scoreRun(directory)).status, "invalid");
  }));

test("invalid scores and tampered input invalidate summary; differing rubrics are not comparable", () =>
  temporary(async (parent) => {
    const directory = parent + "/run";
    await prepareRun("invalid", { directory });
    const data = reader();
    data.scores.flow.score = "3";
    await writeFile(join(directory, "reader.json"), JSON.stringify(data));
    assert.equal((await scoreRun(directory)).status, "invalid");
    await writeFile(join(directory, "reader/rubric.md"), "changed");
    assert.equal((await scoreRun(directory)).status, "invalid");
    const one = parent + "/one",
      two = parent + "/two";
    for (const path of [one, two]) {
      await prepareRun(path === one ? "one" : "two", { directory: path });
      await writeFile(join(path, "reader.json"), JSON.stringify(reader()));
    }
    const hashes = JSON.parse(await readFile(join(two, "inputs.json"), "utf8"));
    hashes["reader/rubric.md"] = "different";
    await writeFile(join(two, "inputs.json"), JSON.stringify(hashes));
    const comparison = await compareRuns(one, two);
    assert.equal(comparison.comparableRubric, false);
    assert.equal(comparison.scoreDelta, null);
  }));

test("reader answers follow the frozen suite contract and older runs remain readable", () =>
  temporary(async (parent) => {
    const directory = join(parent, "syntax");
    await prepareRun("syntax", { directory });
    const manifestPath = join(directory, "manifest.json");
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    const data = reader();
    delete data.answers.syntax;
    await writeFile(join(directory, "reader.json"), JSON.stringify(data));
    manifest.suite.answerKeys = [...Object.keys(data.answers), "syntax"];
    await writeFile(manifestPath, JSON.stringify(manifest));
    await assert.rejects(readReader(directory), /missing answer: syntax/);
    data.answers.syntax = "WAT and WLT syntax distinguished using the cheatsheet";
    await writeFile(join(directory, "reader.json"), JSON.stringify(data));
    await readReader(directory);
    delete manifest.suite.answerKeys;
    delete data.answers.syntax;
    await writeFile(manifestPath, JSON.stringify(manifest));
    await writeFile(join(directory, "reader.json"), JSON.stringify(data));
    await readReader(directory);
  }));

test("embedded standard WLT sources participate in the implementation fingerprint", () =>
  temporary(async (parent) => {
    await mkdir(join(parent, "stdlib"));
    const path=join(parent, "stdlib/types.wlt");
    await writeFile(path, "before");
    const before=await implementationFingerprint(parent);
    assert.ok(before.files["stdlib/types.wlt"]);
    await writeFile(path, "after");
    const after=await implementationFingerprint(parent);
    assert.notEqual(before.sha256, after.sha256);
  }));
