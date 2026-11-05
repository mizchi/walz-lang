// Preserve verification of historical snapshots without duplicating the oracle.
import { resolve } from "node:path";
import { verifyLegacy } from "../evals/lib/runner.mjs";
const [mode, directory, source] = process.argv.slice(2);
if (mode !== "verify" || !directory)
  throw new Error(
    "usage: node runtime/grammar-eval-verify.mjs verify <round-directory> [negative-control-source]",
  );
const result = await verifyLegacy(resolve(directory), source);
console.log(JSON.stringify(result, null, 2));
process.exitCode = result.passed ? 0 : 1;
