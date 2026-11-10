<a id="walz-の評価を繰り返す"></a>

# Repeating Walz evaluations

[English](README.md) | [日本語](README.ja.md)

Use the inventory ledger task to assess first-time comprehension, code readability, and correctness of code changes.
Reusable tasks, rubrics, external contracts, and verifiers are separated here. Do not give previous scores or answers to new evaluators.
Suites are [stock-ledger](stock-ledger/suite.json) and [diagnostics](diagnostics/README.md). The procedure below covers the default inventory-ledger evaluation. Prepare error-message evaluations with `just eval-diagnostics-prepare ID`, then use the same review/verify/score/compare steps.
The independent [struct-literals](struct-literals/README.md) comprehension task compares struct-construction forms, evaluating naming, braces, and expected-type literals separately.
The current inventory-ledger suite asks about control flow, ownership, and code changes, plus the distinction between standard WAT and `@` extensions, `walz:core` basic types, monomorphization, multiple-binding `@let`, and analysis boundaries. Required answers are fixed in each run's suite.answerKeys. The 5-axis rubric and 14 execution checks for each are retained; do not compare score differences across suite versions.
Earlier records remain in [grammar evaluation](../docs/grammar-eval/REPORT.md), [readability](../docs/readability/REPORT.md), and [after grammar improvements](../docs/readability/implemented/REPORT.md).

For an evaluation exercising CLI, LSP, async, native WIT, and complex JSON processing, see the [development-experience trial](usability/README.md). Distinguish trials by implementers already familiar with the system from the first-time comprehension evaluations here.

For authoring, composing, and scaling user-defined syntax, see [macro usability and scale evaluation](#マクロの実用性と規模の評価). It covers comparisons between ordinary functions and macros, unsupported forms, and expansions concentrated in one function.

<a id="1-新しい評価回を作る"></a>

## 1. Create a new evaluation run

In `walz-lang`, use Node 24 or later and the existing development dependencies. Execution checks require Cargo, Lean (verified exports), and Wasm GC/JSPI.

```sh
just eval-prepare trial-01
# 任意の場所にも作成できる。固定の階層深さや個人のパスに依存しない。
node evals/run.mjs prepare trial-02 --output /tmp/walz-eval-trial-02
```

Existing runs are not overwritten. IDs have no run-count limit, and `evals/runs/` is ignored by Git.
The current CHEATSHEET, main, line-numbered version, contracts, criteria, tasks, and dependent .walz files are copied into support.
Since suite v8, required public names' types, constraints, and effects are extracted from generated standard API documentation and fixed as `api.md` for both roles.
v9 standardizes effect-contract notation on `@effect`. Existing run inputs, answers, and records remain unchanged.
v10 standardizes standard type names on `$string`. Tasks and checks remain those of v9, with inputs fixed in new runs.
v11 uses WIT-aligned standard types, related functions, and `@string` / `@tuple` / `@box` vocabulary.
v12 groups JSON operations under `$json.*`. Task execution contracts remain intact; past inputs and results stay unchanged.
v16 standardizes on `@use` from `walz:core` and relative file modules, without `@namespace`. Distributed grammar documentation includes threads and the verification mode for fixed atomic cells. Old-suite distributions and pre-rename input archives remain unchanged.
v19 uses the `@` extension prefix, `.walz` sources, `walz:core`, English reference documentation, and the `walz/core/` support layout. The preceding WLT suites are saved in [versions/v18.json](stock-ledger/versions/v18.json), [versions/v17.json](stock-ledger/versions/v17.json), and [versions/v16.json](stock-ledger/versions/v16.json); past run inputs and results remain unchanged. Current macro and usability harnesses record input revision 4 and `language: "walz"` in new runs.
main's include references support from the copied program.walz, so later library changes do not alter evaluation code.
When adding dependencies, also update support in suite.json.

Main artifacts:

```text
evals/runs/trial-01/
  dispatch.md           # 各初見エージェントへ渡す指示
  manifest.json         # suite、環境、実装fingerprint、役ごとの読取り範囲
  inputs.json           # 変更しない配布物・依存ソースのSHA-256
  reader/              # task、cheatsheet、api、契約、基準、main、counted
  writer/              # task、cheatsheet、api、契約、main、candidate.walz、program.walz
  support/             # 静的includeの依存ソース
  reviewer-task.md      # 親による回答監査の手順
```

<a id="2-初見の読解役変更役へ渡す"></a>

## 2. Dispatch to first-time reader and writer roles

Pass the 2 instructions in dispatch.md to separate contexts with no conversation history. The CLI itself does not create agents.
Read/write restrictions are evaluation instructions, not an OS sandbox.

- The **reader** reads only the distributions under reader and writes answers, 5-axis scores, and evidence to reader.json. answers.syntax / bindings / standard explain grammar, initialization order, and standard types; self-scoring alone does not establish correctness. The reader does not run the compiler.
- The **writer** edits candidate.walz based on main under writer, changing the success value to the delivered-line count. CLI check/types/fmt are allowed. Results go in writer/report.json.

The v8 writer also adds `count64` and `format-count` to the same candidate. The former checks changes to fold state, constants, operations, and success type to i64; the latter checks unsigned-integer parsing and text generation through selected standard APIs.

Do not give the writer the completed counted example, reader materials, dependency implementations, or past results. Ordinary helpers are allowed, but the requirement to keep state in a pair remains.
The initial candidate is main with success value 0 and fails execution checks if unchanged. Copying the template alone cannot pass.

<a id="3-回答を監査し実行集計する"></a>

## 3. Audit answers, execute, and aggregate

The parent or a separate auditor reads reviewer-task.md and checks reader.json explanations against the distributed code.
Self-scoring alone does not pass. Record concrete reasons for the audit judgment.

```sh
node evals/run.mjs review trial-01 --correct --reason '型・所有性・EOFとerrの説明を、配布したmainと契約の該当箇所へ照合した'
# 誤答なら --correct の代わりに --incorrect。
just eval-verify trial-01
just eval-score trial-01
```

A run is an ID or path. The same commands work with `/tmp/...` or `./somewhere/run`.
review fixes the answer hash. Revised answers require another audit; revised candidates require another execution.
If distributed materials or dependencies change, start a new prepare rather than appending to the existing run.

verify independently builds the reader's counted and writer's candidate and runs 14 checks each.
It checks types/Wasm validation, pair state, exactly 1 initialization, delivered-line counts and JSON, EOF/I/O errors, cancellation during reads/writes, handle tables, and ABI live=0.
This is not a proof that the entire arbitrary GC heap is leak-free. For regression checks when Component/browser behavior also changes, use existing `just test-stock-ledger` / `just test-e2e` as well.

To stop synchronous Wasm infinite loops too, the entire child process terminates after 30 seconds by default. Adjust for the environment with `verify RUN --timeout-ms 120000`.
Build artifacts go in work/; verification results in verification.json. The implementation fingerprint and whether it changed after prepare are also recorded.
Bundled `walz:core` sources are included in the fingerprint as part of the implementation. For v7 candidates, existing W4 also checks that main's initialization uses multiple-binding `@let`.
v8 adds candidate checks W5 (i64 fold state and execution) and W6 (standard APIs, integer limits, invalid inputs, and ABI live=0), checking 14 sample cases and 16 candidate cases. Negative harness cases reject implementations that only convert the final value to i64 or return fixed text.
The compiler/runtime itself is not copied, so exact reproduction requires the recorded implementation and toolchain. Past sources can also be reverified with the current implementation.

summary.json status:

| status | Condition | score exit code |
|---|---|---:|
| pass | Every axis is at least 3, answer audit passes, writer is complete, and both programs pass all execution checks | 0 |
| fail | Required results are present but conditions are unmet | 1 |
| incomplete | Answers, writer report, audit, or execution are incomplete, or results are stale | 2 |
| invalid | Invalid JSON structure or scores, or mismatched distribution hashes | 1 |

Do not award readability points for successful execution or offset 1 weak axis with a high total score.

<a id="4-改善して次の回と比較する"></a>

## 4. Improve and compare with the next run

```sh
just eval-prepare trial-02
# 新しい初見評価役で同じ手順を実施
just eval-compare trial-01 trial-02
```

Comparisons emit JSON containing scores, pass/fail, per-axis differences, and source metrics (token counts, depth, and repeated types). Score differences are omitted when suite/rubric hashes differ.
Do not relax criteria midway. Record findings → causes → code/documentation fixes → machine verification → first-time evaluation.
These are a small number of subjective scores from the same model; do not interpret differences as human working time or statistical effects.
Required results can be saved as whole directories and reverified elsewhere while preserving relative includes.

For saved evaluations, see [v6 with `%` syntax](reports/20261106-prefix-v6/REPORT.md) and [v7 with standard-type imports and multiple bindings](reports/20261106-standard-bindings-v7/REPORT.md). Each includes distributed materials, reader answers, writer candidates, audits, and execution results.
The [v8 evaluation record](reports/20261106-readability-v8/REPORT.md) stores formatting, naming, and API-documentation fixes and results of the added API-selection/i64-conversion tasks.

<a id="マクロの実用性と規模の評価"></a>

## Macro usability and scale evaluation

These are unblinded trials by implementers, distinct from measuring independent first-time readers' comprehension or human working time.

| suite | What it exercises | Command |
| --- | --- | --- |
| [Macros v1](macro-usability/README.md) | 3 JSON-application implementations, CLI, Core＋JSPI/Component ABI cleanup/cancellation, stdio LSP, and expansion scale | `just eval-user-macros ID` |
| [Authoring and scale v2](macro-usability-v2/README.md) | Ordinary functions and macros using type inference/public type information, binding composition, concentrated and distributed expansion | `just eval-macro-authoring ID` |

Use a separate new ID for each. Requires Node.js 24 or later, Rust 1.99, Wasm GC-capable Wasmtime, and existing development dependencies. The 2 suites have different tasks, so do not compare score differences or claim v2 alone checks ABI, cancellation, and editor behavior.

```sh
just eval-user-macros my-macros-v1-01
just eval-macro-authoring my-macros-v2-01
```

Tasks, expectations, and materials are fixed in `snapshot/` and `inputs.json`; implementation hashes are recorded in `manifest.json`. `commands.json` contains arguments, exit codes, output, and times; `result.json` contains judgments and measurements; `work/` contains artifacts such as Wasm. Output goes in `evals/runs/ID/`. When saving required records, include ignored `work/` too, retaining artifacts referenced by hashes.

Exploratory tasks may remain unmet even after required checks pass. Read v1's `unmetTasks` and v2's `unmet` too. Distinguish specification-based rejection of exploratory tasks from misbehavior of accepted programs; the latter is a correctness failure even for exploratory tasks. v2 runs every accepted program in both Wasmtime and V8.

Do not fix the implementation mid-evaluation and append results. After changes, run under another ID; changes to tasks also require a new suite version. Times include CLI startup/loading; a few short measurements do not establish universal speed or human productivity.

Records after resuming checking from expressions are [v1 r4](reports/20261109-macro-usability-v1-r4/REPORT.md) and [v2 r3](reports/20261109-macro-authoring-v2-r3/REPORT.md). The existing 20 checks and 11 additional required checks passed, as did 512 expansions in one function. The 3 remaining tasks—sequential initialization, Result repetition templates, and delegation wrapping bodies—are recorded as limitations of current scope and syntax.

<a id="ハーネスのテスト"></a>

## Harness tests

```sh
just test-evals
```

Checks cover refusal to overwrite preparation, modified distributions, role separation, score formats, invalidation after audit/candidate changes, mismatched criteria, actual positive/negative cases, and synchronous Wasm timeouts.
Old runtime/grammar-eval-prepare.mjs and verify.mjs remain compatibility entry points; the verifier implementation is consolidated in stock-ledger/verify.mjs. Old prepare now uses the new output layout.
