// Compatibility entry point. New workflows live under evals/.
import { join } from "node:path";
import { prepareRun, projectRoot } from "../evals/lib/runs.mjs";
const round = process.argv[2];
if (!/^[0-9]+$/.test(round ?? ""))
  throw new Error(
    "usage: node runtime/grammar-eval-prepare.mjs <round-number>",
  );
const directory = await prepareRun(`grammar-${round}`, {
  directory: join(projectRoot, "target/grammar-eval", `round-${round}`),
});
console.log(directory + "\n" + directory + "/dispatch.md");
