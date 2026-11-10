<a id="rustフロントエンドの計測"></a>

# Measuring the Rust frontend

Uses Criterion 0.8.2 as a development dependency to measure Walz type checking and compilation to Core Wasm.
These measurements are separate from Cargo/rustc build time. They use Rust's bench profile and exclude Wasm execution.

See the [20261107-r1 measurement record](../benchmarks/frontend/20261107-r1/report.md) for the initial improvements.

```sh
just test-frontend
just frontend-save before-r1
# Rust側を修正し、機能検査を通す
just frontend-compare before-r1
just frontend-bench '^check/std-types$'
```

`frontend-save` does not overwrite existing labels. Comparisons preserve the baseline and, by default,
save results under `target/criterion/frontend-v7/`. Setting `CRITERION_HOME` changes the base directory, but results remain separated under its `frontend-v7/` subdirectory.
For direct invocation options, see [Criterion's baseline documentation](https://bheisler.github.io/criterion.rs/book/user_guide/command_line_options.html).

<a id="対象と条件"></a>

## Targets and conditions

Inputs in `benches/fixtures/frontend-v7/` are fixed. V7 prefixes match arms with `@case`; v1–v6 inputs and references retain their original bytes. Reference checks compare v7 WAT exactly with v6/v5 and normalize only generated names/host namespaces when comparing earlier language names. IR, analysis, and diagnostic references are captured separately for v7.
Large inputs fix the compiler's internal form after file resolution. Because they contain private scope representations, CLI reference comparisons explicitly use the legacy fixture path; they are not public-source syntax examples.
Application file imports/includes are expanded in advance,
so API measurements exclude file reads. Create a new version when changing inputs.

| group | Target |
| --- | --- |
| `check` | Name, type, effect, and ownership checks. Native WAT, let, standard types, strings, JSON, inventory ledger, workflow |
| `compile` | The checks above, analysis, optimization/DCE, WAT/Wasm generation, and the Wasm validator |
| `check-error` | Diagnostics for type errors referencing the standard library |
| `cold-check` | check in a new CLI process each time, including startup and file I/O. The OS file cache warms up |

Defaults are 30 samples, 500 ms warmup, and a 2-second target measurement duration. Inputs and results are passed to `std::hint::black_box`,
and releasing generated IR and similar data is included in each iteration's time. Measurement settings can be changed through the CLI.
Standard library vocabulary is stored in a build-time index; syntax trees are held in a fixed-size, process-local read-only cache.
The standard library cache retains no user source, import aliases, type-check results, policies, or ownership state. The LSP has a separate bounded IR cache that distinguishes revisions.
Check repeated API calls and fresh-process time separately.

<a id="ネイティブプロファイル"></a>

## Native profiles

On macOS, select one target to collect Rust stacks. Use a new output destination each time.

```sh
just frontend-profile check/std-types target/frontend-profile/check-r1.txt
just frontend-profile compile/workflow target/frontend-profile/compile-r1.txt
```

This task repeats the same operation using Criterion's `--profile-time` and runs macOS `sample` at 1 ms intervals.
Cargo builds complete before collection. Do not use profiled times for speed comparisons.
On Linux and similar platforms, pass `--bench --profile-time` to the benchmark executable and collect with perf or a similar tool.
For flag meanings, see [Criterion's profiling documentation](https://bheisler.github.io/criterion.rs/book/user_guide/profiling.html).

<a id="正しさの検査"></a>

## Correctness checks

`runtime/frontend-reference.test.mjs` checks fixed pre-change inputs, typed IR, analysis results, WAT, and diagnostics using SHA-256 and other comparisons.
Do not update `tests/fixtures/frontend-reference-v7.json` for speed improvements.
The pre-rename v1 reference is retained too, and generated WAT is checked for equality apart from internal namespace names.
Intentional API changes require new versions of inputs and references.
Rust tests also verify that using only standard types does not parse unrelated libraries, that generated indexes match their sources,
and that concurrent calls isolate aliases and hygiene.

<a id="abi-型グラフの検査"></a>

## ABI type graph checks

Value ABI layouts, flatten counts, and type representations are memoized by type ID. DAGs containing the same type twice are not repeatedly expanded as trees. Adapter flattening and WIT named-type generation reuse results too. Checked arithmetic and [ABI limits](wit-bindings.md#abi-型の実装上限) terminate oversized expansions with diagnostics.

`tests/abi_limits.rs` checks duplicate DAGs and rejection boundaries, while deterministic visit-count tests in lib prevent rechecking each vertex. No processing-time improvement ratio is claimed here. For comparisons, keep existing inputs and baselines and use a new measurement label.

<a id="マクロ展開後の型検査"></a>

## Type checking after macro expansion

Checking resumes from the expanded expression without rechecking completed outer or preceding expressions. Local IDs created by trial type queries are discarded, and only syntax is reused. A new instantiation that changes the type environment triggers a full recheck. Expression resumption, final body size/depth, and whole-program contract checks are separate. See the [implementation structure](architecture.md) and [checking conventions](user-macros.md#checking-scope-and-work-counters).

The `checking` output from `macro-expand` exposes module-checking pass counts, function-body entry counts, and budget-counted syntax visits. These are neither total checker operations nor CPU time, and are distinct from Criterion measurements.

In the [20261109 comparison](../evals/reports/20261109-macro-authoring-v2-r3/REPORT.md) using the same fixed inputs, 256 expansions in one function dropped from 776.51 ms to 35.22 ms, and node visits from 1,317,396 to 9,751. The 512-expansion case, which previously exceeded the two-million-node budget, succeeded in 77.55 ms without changing the budget. Generated Wasm was identical for all 12 programs accepted both before and after.

These are CLI medians over three runs each, including startup and loading. No equivalent speedup is claimed for distributed expansions, and these results are not guest execution speed comparisons. Remeasure with a new ID; do not overwrite existing inputs, rubrics, or results.

```sh
just eval-user-macros my-macros-v1-01
just eval-macro-authoring my-macros-v2-01
```

v1 tests applications including ABI, cancellation, and LSP; v2 tests macro authoring with the current API and expansion into one or multiple functions. The [evaluation guide](../evals/README.md#マクロの実用性と規模の評価) explains the differences in saved artifacts and judgments.
