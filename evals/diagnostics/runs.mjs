import {
  mkdir,
  readFile,
  writeFile,
  copyFile,
  cp,
  rm,
  realpath,
} from "node:fs/promises";
import { resolve, join, dirname, relative, isAbsolute } from "node:path";
import { execFile as rawExec } from "node:child_process";
import { promisify, stripVTControlCharacters } from "node:util";
import {
  projectRoot,
  evalRoot,
  sha256,
  json,
  save,
  optional,
  files,
  command,
  checkInputs,
  implementationFingerprint,
  toolVersions,
} from "../lib/runs.mjs";
import { functionForms, sourceIncludes } from "../lib/source.mjs";
const exec = promisify(rawExec),
  sourceRoot = join(evalRoot, "diagnostics");
const safe = (file) =>
  file && !isAbsolute(file) && !file.split(/[\\/]/).includes("..");
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

async function compiler() {
  const output = await exec(
    "cargo",
    ["build", "--locked", "--quiet", "--bin", "wlt", "--message-format=json"],
    { cwd: projectRoot, timeout: 120000, maxBuffer: 8 * 1024 * 1024 },
  );
  const artifact = output.stdout
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line))
    .findLast(
      (x) =>
        x.reason === "compiler-artifact" &&
        x.target.name === "wlt" &&
        x.executable,
    );
  if (!artifact) throw new Error("Cargo did not report the wlt executable");
  return {
    path: artifact.executable,
    sha256: sha256(await readFile(artifact.executable)),
  };
}
async function invoke(cli, args, root) {
  let result;
  try {
    const out = await exec(cli.path, [...(cli.legacySyntax ? ["--legacy-syntax"] : []), ...args], {
      cwd: projectRoot,
      timeout: 20000,
      maxBuffer: 2 * 1024 * 1024,
    });
    result = {
      exitCode: 0,
      stdout: out.stdout,
      stderr: out.stderr,
      signal: null,
      timedOut: false,
    };
  } catch (e) {
    if (typeof e.code !== "number" && !e.signal) throw e;
    result = {
      exitCode: typeof e.code === "number" ? e.code : null,
      stdout: e.stdout ?? "",
      stderr: e.stderr ?? "",
      signal: e.signal ?? null,
      timedOut: !!e.killed,
    };
  }
  const roots = [root, await realpath(root)].sort(
    (a, b) => b.length - a.length,
  );
  const normalize = (text) =>
    roots.reduce(
      (text, path) => text.replaceAll(path + "/", ""),
      stripVTControlCharacters(text),
    );
  return {
    ...result,
    stdout: normalize(result.stdout),
    stderr: normalize(result.stderr),
  };
}
function locations(text) {
  return [...text.matchAll(/([^\s"'<>]+\.wlt):(\d+):(\d+):?/gu)].map((m) => ({
    file: m[1],
    line: Number(m[2]),
    column: Number(m[3]),
  }));
}
async function originalFacts(c, packet, directory) {
  const reported = locations(packet.stdout + "\n" + packet.stderr);
  const expected = [];
  for (const target of c.locations) {
    const source = await readFile(
      join(directory, "corpus", c.id, "bad", target.file),
      "utf8",
    );
    const offset = source.indexOf(target.marker);
    if (offset < 0)
      throw new Error("missing diagnostic target marker: " + c.id);
    expected.push({
      file: target.file,
      line: source.slice(0, offset).split("\n").length,
    });
  }
  const valid = [];
  for (const point of reported) {
    if (!safe(point.file)) continue;
    try {
      const lines = (
        await readFile(
          join(directory, "corpus", c.id, "bad", point.file),
          "utf8",
        )
      ).split("\n");
      if (
        point.line >= 1 &&
        point.line <= lines.length &&
        point.column >= 1 &&
        point.column <= [...lines[point.line - 1]].length + 1
      )
        valid.push(point);
    } catch (e) {
      if (e.code !== "ENOENT") throw e;
    }
  }
  const noPanic = !/panicked at|stack backtrace:|internal compiler error/i.test(
    packet.stderr + packet.stdout,
  );
  const rejected =
    packet.exitCode === 1 &&
    !packet.signal &&
    !packet.timedOut &&
    !!packet.stderr.trim();
  return {
    rejected,
    noPanic,
    locations: reported,
    expectedLocations: expected,
    locationPassed: valid.some((p) =>
      expected.some((e) => e.file === p.file && e.line === p.line),
    ),
    terms: c.terms.map((term) => ({
      term,
      present: (packet.stderr + packet.stdout).includes(term),
    })),
  };
}
async function render(name, values) {
  return (await readFile(join(sourceRoot, name + ".md"), "utf8")).replace(
    /\{\{([A-Z_]+)\}\}/g,
    (_, key) => values[key],
  );
}
export async function candidateHashes(directory) {
  const out = {};
  for (const file of await files(join(directory, "writer/cases"))) {
    const name = relative(directory, file);
    out[name] = sha256(await readFile(file));
  }
  return out;
}
export async function prepareDiagnostics(
  id,
  { directory, root = projectRoot },
) {
  if (!/^[\p{L}\p{N}][\p{L}\p{N}._-]*$/u.test(id))
    throw new Error("run ID must be a name without path separators");
  directory = resolve(directory);
  await mkdir(dirname(directory), { recursive: true });
  await mkdir(directory);
  try {
    const suite = await json(join(sourceRoot, "suite.json"));
    if (
      new Set(suite.cases.map((c) => c.id)).size !== suite.cases.length ||
      !suite.cases.length
    )
      throw new Error("diagnostic case IDs must be unique");
    for (const c of suite.cases)
      if (
        !Number.isInteger(c.tests) ||
        c.tests < 1 ||
        !c.editable?.length ||
        !c.locations?.length ||
        c.locations.some((x) => !safe(x.file) || !x.marker) ||
        (c.guardFile && !safe(c.guardFile))
      )
        throw new Error("invalid diagnostic case contract: " + c.id);
    const implementation = await implementationFingerprint();
    const cli = await compiler(),
      mutable = new Set(),
      corpus = {};
    for (const role of ["reader", "writer"]) {
      await mkdir(join(directory, role), { recursive: true });
      await copyFile(
        join(root, "CHEATSHEET.md"),
        join(directory, role, "cheatsheet.md"),
      );
    }
    await copyFile(
      join(sourceRoot, "rubric.md"),
      join(directory, "reader/rubric.md"),
    );
    await copyFile(
      join(sourceRoot, "suite.json"),
      join(directory, "suite.json"),
    );
    for (const c of suite.cases) {
      if (!/^[a-z0-9-]+$/.test(c.id) || c.editable.some((f) => !safe(f)))
        throw new Error("invalid diagnostic case path");
      const target = join(directory, "corpus", c.id);
      await cp(join(sourceRoot, "cases", c.fixture), target, {
        recursive: true,
      });
      for (const file of await files(target))
        corpus[relative(join(directory, "corpus"), file)] = sha256(
          await readFile(file),
        );
      for (const role of ["reader", "writer"]) {
        const packetRoot = join(directory, role, "cases", c.id);
        await cp(join(target, "bad"), packetRoot, { recursive: true });
        await writeFile(
          join(packetRoot, "intent.md"),
          `# ${c.id}\n\n${c.intent}\n\n編集可能: ${c.editable.join(", ")}。その他のソースは変更不可。\n`,
        );
      }
      for (const f of c.editable) {
        await readFile(join(target, "bad", f));
        mutable.add(`writer/cases/${c.id}/${f}`);
      }
      const packet = await invoke(
        cli,
        ["check", join(target, "bad/program.wlt")],
        join(target, "bad"),
      );
      if (packet.signal || packet.timedOut)
        throw new Error("compiler failed to collect diagnostic: " + c.id);
      for (const role of ["reader", "writer"])
        await save(
          join(directory, role, "cases", c.id, "diagnostic.json"),
          packet,
        );
      await mkdir(join(directory, "private/drivers"), { recursive: true });
      const template = await readFile(join(target, "tests.wlt"), "utf8");
      for (const [kind, program] of [
        ["control", join(target, "fixed/program.wlt")],
        ["repair", join(directory, "writer/cases", c.id, "program.wlt")],
      ]) {
        const driver = join(
          directory,
          "private/drivers",
          c.id + "-" + kind + ".wlt",
        );
        const include = relative(dirname(driver), program).replaceAll(
          "\\",
          "/",
        );
        await writeFile(
          driver,
          template.replace("{{PROGRAM}}", JSON.stringify(include)),
        );
      }
    }
    if (
      (await implementationFingerprint()).sha256 !== implementation.sha256 ||
      sha256(await readFile(cli.path)) !== cli.sha256
    )
      throw new Error("compiler changed while collecting diagnostics");
    const values = {
      READER_OUTPUT: join(directory, "reader.json"),
      WRITER_OUTPUT: join(directory, "writer/report.json"),
      PROJECT_ROOT: root,
      REVIEW_COMMAND: command([
        "node",
        join(evalRoot, "run.mjs"),
        "review",
        directory,
        "--correct",
        "--reason",
        "全ケースの原因・修正・診断中の情報を照合した具体的な根拠",
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
    for (const role of ["reader", "writer", "reviewer"])
      await writeFile(
        join(
          directory,
          role === "reviewer" ? "reviewer-task.md" : role + "/task.md",
        ),
        await render(role, values),
      );
    const roles = {};
    for (const role of ["reader", "writer"])
      roles[role] = (await files(join(directory, role))).map((f) =>
        relative(directory, f),
      );
    await save(join(directory, "manifest.json"), {
      schema: "wlt-eval-v1",
      id,
      suite,
      preparedAt: new Date().toISOString(),
      projectRoot: root,
      implementation,
      versions: await toolVersions(),
      compiler: cli,
      corpusFingerprint: sha256(JSON.stringify(corpus)),
      roles,
    });
    await writeFile(
      join(directory, "dispatch.md"),
      `# 診断品質の評価\n\n会話履歴・過去の結果を持たない別の役へ渡す。\n\n- 読解役: ${join(directory, "reader/task.md")} を読み指示だけに従う。\n- 修正役: ${join(directory, "writer/task.md")} を読み指示だけに従う。\n\n親が reviewer-task.md で監査し、verify / score を実行する。正解・非公開テストを初見の役へ渡さない。CLIはエージェントを生成しない。\n`,
    );
    const hashes = {};
    for (const f of await files(directory)) {
      const name = relative(directory, f);
      if (!mutable.has(name)) hashes[name] = sha256(await readFile(f));
    }
    await save(join(directory, "inputs.json"), hashes);
    return directory;
  } catch (e) {
    await rm(directory, { recursive: true, force: true });
    throw e;
  }
}

async function testProgram(cli, directory, c, kind) {
  const program =
    kind === "control"
      ? join(directory, "corpus", c.id, "fixed/program.wlt")
      : join(directory, "writer/cases", c.id, "program.wlt");
  if (kind === "repair") {
    const root = dirname(program),
      expected = (await files(join(directory, "corpus", c.id, "bad"))).map(
        (f) => relative(join(directory, "corpus", c.id, "bad"), f),
      );
    const actual = (await files(root))
      .map((f) => relative(root, f))
      .filter((f) => !["intent.md", "diagnostic.json"].includes(f));
    if (!same(expected, actual))
      return {
        passed: false,
        behaviorPassed: false,
        invariantsPassed: false,
        error: "only supplied source files may be edited",
      };
    for (const file of actual) {
      try {
        const include = sourceIncludes(
          await readFile(join(root, file), "utf8"),
        );
        if (
          include.some(
            (p) =>
              !expected.includes(
                relative(root, resolve(dirname(join(root, file)), p)),
              ),
          )
        )
          return {
            passed: false,
            behaviorPassed: false,
            invariantsPassed: false,
            error: "include must remain within supplied case files",
          };
      } catch (e) {
        return {
          passed: false,
          behaviorPassed: false,
          invariantsPassed: false,
          error: "case source cannot be inspected: " + e.message,
        };
      }
    }
  }
  const check = await invoke(cli, ["check", program], dirname(program));
  const tests =
    check.exitCode === 0
      ? await invoke(
          cli,
          [
            "test",
            join(directory, "private/drivers", c.id + "-" + kind + ".wlt"),
            "--timeout-ms",
            "1000",
          ],
          directory,
        )
      : null;
  const counts = tests?.stdout.match(
    /(\d+) passed; (\d+) failed; (\d+) errors;/,
  );
  const behaviorPassed = !!(
    tests &&
    tests.exitCode === 0 &&
    counts &&
    Number(counts[1]) >= c.tests &&
    counts[2] === "0" &&
    counts[3] === "0"
  );
  let invariantsPassed = true;
  if (Object.keys(c.guards).length) {
    if (check.exitCode !== 0) invariantsPassed = false;
    else {
      const source = await readFile(
        join(dirname(program), c.guardFile ?? "program.wlt"),
        "utf8",
      );
      for (const [name, counts] of Object.entries(c.guards)) {
        const forms = functionForms(source, name);
        if (
          !forms ||
          Object.entries(counts).some(
            ([head, count]) => (forms[head] ?? 0) !== count,
          )
        )
          invariantsPassed = false;
      }
    }
  }
  return {
    passed: check.exitCode === 0 && behaviorPassed && invariantsPassed,
    check,
    tests,
    behaviorPassed,
    invariantsPassed,
  };
}
export async function verifyDiagnostics(directory) {
  const hashes = await candidateHashes(directory);
  try {
    const inputsFingerprint = await checkInputs(directory),
      manifest = await json(join(directory, "manifest.json"));
    const implementation = await implementationFingerprint(),
      cli = { ...await compiler(), legacySyntax: manifest.suite.version < 4 },
      cases = {};
    let diagnosticsCurrent = true;
    for (const c of manifest.suite.cases) {
      const actual = await invoke(
        cli,
        ["check", join(directory, "corpus", c.id, "bad/program.wlt")],
        join(directory, "corpus", c.id, "bad"),
      );
      const packet = await json(
        join(directory, "reader/cases", c.id, "diagnostic.json"),
      );
      const current = same(actual, packet);
      diagnosticsCurrent &&= current;
      const original = await originalFacts(c, actual, directory);
      const control = await testProgram(cli, directory, c, "control");
      const repair = await testProgram(cli, directory, c, "repair");
      cases[c.id] = {
        passed:
          original.rejected &&
          original.noPanic &&
          original.locationPassed &&
          control.passed &&
          repair.passed &&
          current,
        original,
        control,
        repair,
        diagnosticMatchesPacket: current,
      };
    }
    if (!same(hashes, await candidateHashes(directory)))
      throw new Error("repair source changed during verification");
    await checkInputs(directory);
    if (
      (await implementationFingerprint()).sha256 !== implementation.sha256 ||
      sha256(await readFile(cli.path)) !== cli.sha256
    )
      throw new Error("compiler changed during verification");
    const out = {
      schema: "wlt-diagnostics-verification-v1",
      passed: Object.values(cases).every((c) => c.passed),
      inputsFingerprint,
      implementation,
      compiler: cli,
      toolchain: await toolVersions(),
      node: process.version,
      candidateHashes: hashes,
      diagnosticsCurrent,
      implementationChangedSincePrepare:
        implementation.sha256 !== manifest.implementation.sha256,
      cases,
    };
    await save(join(directory, "verification.json"), out);
    return out;
  } catch (e) {
    const out = {
      passed: false,
      fatal: e.message,
      node: process.version,
      implementation: await implementationFingerprint(),
      candidateHashes: hashes,
      inputsFingerprint: await checkInputs(directory).catch(() => null),
      diagnosticsCurrent: false,
      cases: {},
    };
    await save(join(directory, "verification.json"), out);
    return out;
  }
}

export async function readDiagnosticReader(directory) {
  const data = await json(join(directory, "reader.json")),
    manifest = await json(join(directory, "manifest.json"));
  const ids = manifest.suite.cases.map((c) => c.id);
  if (
    Object.keys(data.cases ?? {})
      .sort()
      .join() !== ids.sort().join()
  )
    throw new Error("missing or unknown diagnostic case answers");
  for (const id of ids) {
    const answer = data.cases[id];
    for (const key of ["cause", "fix", "explanation"])
      if (typeof answer[key] !== "string" || !answer[key].trim())
        throw new Error("missing diagnostic answer: " + id + "/" + key);
    if (
      Object.keys(answer.scores ?? {})
        .sort()
        .join() !== [...manifest.suite.axes].sort().join()
    )
      throw new Error("invalid diagnostic score axes: " + id);
    for (const key of manifest.suite.axes) {
      const value = answer.scores[key];
      if (
        !Number.isInteger(value?.score) ||
        value.score < 0 ||
        value.score > 4 ||
        typeof value.evidence !== "string" ||
        !value.evidence.trim()
      )
        throw new Error("invalid diagnostic score: " + id + "/" + key);
    }
  }
  if (!Array.isArray(data.issues) || !Array.isArray(data.limitations))
    throw new Error("diagnostic issues/limitations must be arrays");
  return data;
}
export async function scoreDiagnostics(directory) {
  let result;
  try {
    const inputsFingerprint = await checkInputs(directory),
      manifest = await json(join(directory, "manifest.json"));
    const reader = (await optional(join(directory, "reader.json")))
      ? await readDiagnosticReader(directory)
      : null;
    const writer = await optional(join(directory, "writer/report.json")),
      review = await optional(join(directory, "review.json")),
      execution = await optional(join(directory, "verification.json"));
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
    const current = !!(
      execution &&
      execution.implementation?.sha256 ===
        (await implementationFingerprint()).sha256 &&
      execution.node === process.version &&
      execution.inputsFingerprint === inputsFingerprint &&
      same(execution.candidateHashes, await candidateHashes(directory)) &&
      execution.diagnosticsCurrent === true
    );
    const scores = reader
      ? Object.fromEntries(
          manifest.suite.cases.map((c) => [
            c.id,
            Object.fromEntries(
              manifest.suite.axes.map((a) => [
                a,
                reader.cases[c.id].scores[a].score,
              ]),
            ),
          ]),
        )
      : null;
    const meetsTarget =
      !!scores &&
      Object.values(scores).every((s) =>
        Object.values(s).every((v) => v >= manifest.suite.minimumPerAxis),
      );
    const machinePassed = !!(
      current &&
      execution.passed === true &&
      Object.keys(execution.cases ?? {}).length ===
        manifest.suite.cases.length &&
      manifest.suite.cases.every((c) => {
        const v = execution.cases[c.id];
        return (
          v?.passed === true &&
          v.original?.rejected === true &&
          v.original?.noPanic === true &&
          v.original?.locationPassed === true &&
          v.control?.passed === true &&
          v.repair?.passed === true
        );
      })
    );
    const incomplete = !reader || !writer || !reviewed || !current;
    const passed = !!(
      !incomplete &&
      meetsTarget &&
      review.correct &&
      machinePassed &&
      writer.completed &&
      writer.extraDocumentsRead.length === 0
    );
    result = {
      schema: "wlt-eval-summary-v1",
      id: manifest.id,
      suite: manifest.suite.id,
      status: incomplete ? "incomplete" : passed ? "pass" : "fail",
      passed,
      inputsFingerprint,
      quality: {
        scores,
        total: scores
          ? Object.values(scores)
              .flatMap(Object.values)
              .reduce((a, b) => a + b, 0)
          : null,
        meetsTarget,
      },
      answers: { reviewed, correct: reviewed ? review.correct : null },
      execution: { current, passed: machinePassed },
      notes: incomplete
        ? [
            "回答・監査・修正報告・実行が未完了、または診断/修正/実装が更新前です。診断文が変わったら新しいprepareから評価する。",
          ]
        : [],
    };
  } catch (e) {
    result = {
      schema: "wlt-eval-summary-v1",
      status: "invalid",
      passed: false,
      error: e.message,
    };
  }
  await save(join(directory, "summary.json"), result);
  return result;
}
export async function compareDiagnostics(before, after) {
  const a = await scoreDiagnostics(before),
    b = await scoreDiagnostics(after),
    ma = await json(join(before, "manifest.json")),
    mb = await json(join(after, "manifest.json")),
    ah = await json(join(before, "inputs.json")),
    bh = await json(join(after, "inputs.json"));
  const comparableRubric =
    ah["reader/rubric.md"] === bh["reader/rubric.md"] &&
    ah["suite.json"] === bh["suite.json"] &&
    ma.corpusFingerprint === mb.corpusFingerprint;
  return {
    before: a,
    after: b,
    comparableRubric,
    scoreDelta:
      comparableRubric && a.quality?.scores && b.quality?.scores
        ? Object.fromEntries(
            ma.suite.cases.map((c) => [
              c.id,
              Object.fromEntries(
                ma.suite.axes.map((k) => [
                  k,
                  b.quality.scores[c.id][k] - a.quality.scores[c.id][k],
                ]),
              ),
            ]),
          )
        : null,
    note: "ケースと基準を固定した主観評価。点差を人間の作業時間や統計的な効果と扱わない。",
  };
}
