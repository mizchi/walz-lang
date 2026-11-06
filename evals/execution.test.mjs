import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { prepareRun } from "./lib/runs.mjs";
import { verifyRun } from "./lib/runner.mjs";
import { assertSnapshotIncludes, metrics, sourceIncludes, functionForms } from "./lib/source.mjs";

test("source inspection ignores strings/nested comments but rejects live includes", () => {
  assertSnapshotIncludes(String.raw`(module ;; (include "bad")
    (include "program.wlt") (func $f (text.lit "(include \"bad\")")))`);
  assertSnapshotIncludes(
    '(module (; outer (; (include "bad") ;) ;) (include "program.wlt"))',
  );
  assert.throws(
    () => assertSnapshotIncludes('(module (include "../live.wlt"))'),
    /frozen/,
  );
  assert.throws(() => assertSnapshotIncludes("(module)"), /exactly one/);
  const result = metrics(
    "(module (; (( ;) (func $f (result i32) (i32.const 42)))",
  );
  assert.equal(result.maxDepth, 3);
});

test(
  "frozen sources execute outside checkout; seed is a negative control and completed candidate passes",
  { timeout: 60000 },
  async () => {
    const parent = await mkdtemp(join(tmpdir(), "wlt-eval-exec-"));
    try {
      const directory = join(parent, "path with spaces's", "run");
      await prepareRun("integration-99", { directory });
      const negative = await verifyRun(directory);
      assert.equal(negative.passed, false);
      assert.equal(negative.subjects.sample.passed, true);
      const failures = negative.subjects.candidate.checks.filter(
        (c) => !c.passed,
      );
      assert.equal(failures.length, 8);
      assert.ok(
        failures.every(
          (c) => ["W2", "W4", "W5", "W6"].some((prefix) => c.name.startsWith(prefix)),
        ),
      );
      // Parent-only oracle control; writer is never given these sources.
      const sample = await readFile(join(directory, "reader/counted.wlt"), "utf8");
      const extra = await readFile(new URL("stock-ledger/extra-control.wlt", import.meta.url), "utf8");
      const candidate = sample.slice(0, sample.lastIndexOf(")")) + extra.slice(extra.indexOf("(module") + 7);
      await writeFile(join(directory, "writer/candidate.wlt"), candidate);
      const positive = await verifyRun(directory);
      assert.equal(positive.passed, true, JSON.stringify(positive));
      assert.equal(positive.subjects.sample.checks.length, 14);
      assert.equal(positive.subjects.candidate.checks.length, 16);
      const wideStart = candidate.indexOf("  (func $count64");
      const formatStart = candidate.indexOf("  (func $format-count");
      const fakeWide = `(func $count64 (export "count64") (result (%inst $result i64 i32))
        (%effect reads writes allocates host suspends)
        (%result.let ($n) (call $main) (%result.ok (i64.extend_i32_u (local.get $n)))))\n`;
      const falsePositive = (candidate.slice(0, wideStart) + fakeWide + candidate.slice(formatStart))
        .replace('(%result.ok (call $builder.finish (local.get $builder)))', '(%result.ok (%text.lit "count=42"))');
      await writeFile(join(directory, "writer/candidate.wlt"), falsePositive);
      const rejected = await verifyRun(directory);
      assert.equal(rejected.passed, false);
      const extraFailures = rejected.subjects.candidate.checks.filter((c) => !c.passed);
      assert.deepEqual(extraFailures.map((c) => c.name.slice(0, 2)), ["W5", "W6"]);
      await writeFile(join(directory, "support/stdlib/text.wlt"), "changed");
      const tampered = await verifyRun(directory);
      assert.equal(tampered.passed, false);
      assert.match(tampered.fatal, /text.wlt/);
    } finally {
      await rm(parent, { recursive: true, force: true });
    }
  },
);

test(
  "a synchronous Wasm loop is killed and cannot retain a successful verification",
  { timeout: 20000 },
  async () => {
    const parent = await mkdtemp(join(tmpdir(), "wlt-eval-timeout-"));
    try {
      const directory = join(parent, "run");
      await prepareRun("timeout", { directory });
      await writeFile(join(directory, "verification.json"), '{"passed":true}');
      await writeFile(
        join(directory, "writer/candidate.wlt"),
        '(module (include "program.wlt") (func $main (export "main") (result (result i32 i32)) (effects pure) (loop $spin (result (result i32 i32)) (recur $spin))))',
      );
      const result = await verifyRun(directory, { timeoutMs: 5000 });
      assert.equal(result.passed, false);
      assert.match(result.fatal, /timed out/);
    } finally {
      await rm(parent, { recursive: true, force: true });
    }
  },
);

test("prefixed syntax retains evaluation measurements and frozen include checks", () => {
  const old = '(module (include "program.wlt") (func $f (let ($x) (pair (ref $stock.State) i32) (match $V))))';
  const modern = old.replaceAll('(include ', '(%include ').replaceAll('(let ', '(%let ').replaceAll('(pair ', '(%pair ').replaceAll('(match ', '(%match ');
  assertSnapshotIncludes(modern);
  assert.deepEqual(sourceIncludes(modern), ["program.wlt"]);
  assert.deepEqual(metrics(modern), metrics(old));
  assert.deepEqual(functionForms(modern, "$f"), functionForms(old, "$f"));
  assert.throws(() => assertSnapshotIncludes('(module (%include "../live.wlt"))'), /frozen/);
});

test("standard type applications and grouped lets keep distinct lexical measurements", () => {
  const source = '(module (%include "program.wlt") (%import "wlt:std" (type $pair)) (func $main (param $p (%inst $pair (ref $stock.State) i32)) (%let (($a (i32.const 0))) (%let ($x $y) (local.get $p) (nop)))))';
  assertSnapshotIncludes(source);
  const result=metrics(source);
  assert.equal(result.groupedLets, 1);
  assert.equal(result.patternLets, 1);
  assert.equal(result.stateTypeOccurrences, 1);
  assert.equal(functionForms(source, "$main")["let.group"], 1);
  assert.throws(() => assertSnapshotIncludes('(module (%include "program.wlt") (%import "../live.wlt" (func $f)))'), /wlt:std/);
});
