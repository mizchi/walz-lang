import {
  prepareRun,
  resolveRun,
  recordReview,
  scoreRun,
  compareRuns,
} from "./lib/runs.mjs";
import { verifyRun } from "./lib/runner.mjs";
const [action, value, ...args] = process.argv.slice(2);
function option(name) {
  const i = args.indexOf(name);
  return i < 0 ? undefined : args[i + 1];
}
function validateOptions(flags = [], values = []) {
  const seen = new Set();
  for (let i = 0; i < args.length; i++) {
    const key = args[i];
    if (seen.has(key)) throw new Error("duplicate option: " + key);
    seen.add(key);
    if (values.includes(key)) {
      if (args[i + 1] === undefined || args[i + 1].startsWith("--"))
        throw new Error("missing option value: " + key);
      i++;
    } else if (!flags.includes(key)) throw new Error("unknown option: " + key);
  }
}
try {
  if (action === "prepare" && value) {
    validateOptions([], ["--output", "--suite"]);
    const directory = await prepareRun(value, {
      directory: option("--output"),
      suite: option("--suite"),
    });
    console.log(directory + "\n" + directory + "/dispatch.md");
  } else if (action === "verify" && value) {
    validateOptions([], ["--timeout-ms"]);
    const result = await verifyRun(resolveRun(value), {
      timeoutMs:
        option("--timeout-ms") === undefined
          ? 30000
          : Number(option("--timeout-ms")),
    });
    console.log(JSON.stringify(result, null, 2));
    process.exitCode = result.passed ? 0 : 1;
  } else if (action === "review" && value) {
    validateOptions(["--correct", "--incorrect"], ["--reason"]);
    if (args.includes("--correct") === args.includes("--incorrect"))
      throw new Error(
        "select --correct or --incorrect after auditing the answers",
      );
    console.log(
      JSON.stringify(
        await recordReview(resolveRun(value), {
          correct: args.includes("--correct"),
          reason: option("--reason"),
        }),
        null,
        2,
      ),
    );
  } else if (action === "score" && value) {
    validateOptions();
    const result = await scoreRun(resolveRun(value));
    console.log(JSON.stringify(result, null, 2));
    process.exitCode =
      result.status === "pass" ? 0 : result.status === "incomplete" ? 2 : 1;
  } else if (action === "compare" && value && args.length === 1) {
    console.log(
      JSON.stringify(
        await compareRuns(resolveRun(value), resolveRun(args[0])),
        null,
        2,
      ),
    );
  } else
    throw new Error(
      "usage: node evals/run.mjs prepare ID [--suite stock-ledger|diagnostics] [--output DIR] | verify RUN [--timeout-ms N] | review RUN --correct|--incorrect --reason TEXT | score RUN | compare BEFORE AFTER",
    );
} catch (e) {
  console.error(e.message);
  process.exitCode = 1;
}
