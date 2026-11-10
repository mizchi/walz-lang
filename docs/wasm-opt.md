<a id="wasm-opt-に合わせた-wasm-gc-の出力"></a>

# Wasm GC output for wasm-opt

Walz output was examined against ordinary `-O3` in Binaryen 133.
Large `@match` expressions use table branches while preserving externally exposed GC references and trap semantics.
See the [fixed inputs, artifacts, and measurements](../benchmarks/wasm-opt/20261110-v1/report.md).

<a id="どの形が最適化されるか"></a>

## Which forms are optimized

| Input form | Binaryen processing | Walz approach |
| --- | --- | --- |
| Constant arithmetic, unnecessary pure bindings | Constant propagation/folding, unused-local removal | Emit numeric instructions and locals |
| GC structs/Results constructed and only read within a function | `heap2local` can replace allocations with locals, followed by simplification in later passes | Retain `struct.new`/`struct.get` and concrete types |
| Casts to known types, immutable-field reads | `optimize-casts`, `local-subtyping`, common-expression simplification | Retain non-null types, immutable fields, and concrete subtypes |
| Direct calls, unreachable functions | Inlining, removal of unused elements | Retain static callees and explicit exports |
| Large matches comparing tags sequentially | Comparison chains remained after `-O3` on these inputs | Emit `br_table` for eight or more cases |

Pass order, including standard GC passes, was checked in the [133 pass definitions](https://github.com/WebAssembly/binaryen/blob/version_133/src/passes/pass.cpp).
Retaining concrete types and using immutable fields also follows the
[GC lowering tips](https://github.com/WebAssembly/binaryen/wiki/GC-Implementation---Lowering-Tips).
Allocation elimination is limited to cases where references do not escape and observations such as identity are preserved.
There is no guarantee that arbitrary GC objects or ABI values passed to hosts disappear.

In three fixed small examples, unnecessary bindings, local structs, and local Results all became
functions containing only `(i32.const 42)`. Each optimized output is 37 bytes, with a 6-byte code section.
Existing output already supports this optimization, so no duplicate Walz pass was added.

<a id="大きな-match-の出力"></a>

## Output for large matches

Checked enums assign tags `0..N-1` in declaration order, and matches are exhaustive.
For eight or more cases, the subject is stored once in a temporary local, its immutable tag is read once,
and `br_table` selects the case. Table order follows declaration tags, independent of source arm order.
Seven or fewer cases retain the existing `if` chain. Small tables have fixed overhead, so eight is the initial threshold.
This output choice is part of lowering and also applies with Walz `--no-optimize`.

Each case retains the existing `ref.cast` to its concrete subtype.
Cases without payloads also cast. Out-of-range tags go to `unreachable`.
This rejects externally constructed Wasm values with only a base type or mismatched tags and subtypes.
Function evaluation order, return, outer branches, exceptions, unit, and arms returning owned values remain intact.

After `-O3`, total sizes for 8/16/32 cases changed from 496→463, 928→839, and 1792→1591 bytes respectively.
Code sections changed from 292→259, 572→483, and 1132→931 bytes.
Raw Walz output may grow in total size because label names enlarge the name section.
Compare instruction size separately from total size including debug names.

Speed does not necessarily follow size. In V8 comparisons mixing reads of preconstructed references,
32 cases improved by about 5%, 16 cases were nearly unchanged, and eight cases worsened by about 2%.
These small measurements depend on JS calls, tag distributions, JIT, and CPU; they do not guarantee whole-application speedups.

<a id="任意の後処理として実行する"></a>

## Running as optional postprocessing

```sh
walz build input.walz --target core -o target/input.wasm
wasm-opt target/input.wasm \
  --enable-gc --enable-reference-types --enable-exception-handling \
  -O3 -o target/input.opt.wasm
```

Set flags according to the Wasm features used. The commands above target Core Wasm, not whole Components directly.
Normal Walz builds and Playground do not invoke `wasm-opt`. This is also separate from wasm-pack settings for the Rust browser compiler itself.

Running the pipeline more than once can help GC code;
the [official guide](https://github.com/WebAssembly/binaryen/wiki/GC-Optimization-Guidebook) also presents `-O3 -O3`.
For the changed enums here, a second pass saved another five bytes.

Ordinary checks and measurements do not use `--closed-world`, GUFA, `--traps-never-happen`,
`--ignore-implicit-traps`, or fast-math.
closed-world constrains how external code handles GC references; ignoring traps changes when unused expressions can be removed.
Those assumptions differ from these comparisons, which preserve public references and division traps.
See the [133 option definitions](https://github.com/WebAssembly/binaryen/blob/version_133/src/tools/optimization-options.h) for option contracts.

<a id="最適化しにくいソースへの任意警告"></a>

## Optional warnings for sources that are hard to optimize

```sh
walz check examples/optimization/loop-copy.walz --warn-performance
walz check examples/optimization/loop-copy.walz --warn-performance --json
just check-performance examples/optimization/loop-copy.walz
```

After ordinary type/effect/ownership/race/law checks pass, improvement candidates are derived from typed IR evidence.
Compilation errors take priority. Warnings alone do not cause a failing exit code.
Warnings are disabled by default and neither automatically fix sources nor change generated Wasm.

| Code | Detected form | Changes to consider |
| --- | --- | --- |
| `W_OPT_LOOP_COPY` | bytes/list slices in loop bodies, or static callees performing them | Retain original data with indices/ranges, use bytes/string views, or move work outside loops only when semantics are preserved |
| `W_OPT_LOOP_HOST_ESCAPE` | GC values allocated in a loop body of the same function may escape to a host | Pass scalars if the host contract permits, retain GC values within Wasm, or batch transfers at boundaries |

Copy detection uses checked operations and the call graph rather than function names, covering import aliases and instantiated functions.
Copies through helpers include the actual slice position as related information.
Host escape detection is conservative and based on existing region analysis; it does not prove host-side behavior.
It does not cover every repeated path, such as allocations within helpers repeated by caller loops.

Copies only in loop initializers, unreferenced declarations, and locations found unreachable by analysis are excluded.
Inner-loop initialization repeated by an outer loop is included.
Mutable fields and nullable types alone do not trigger warnings.
Identical warnings at the same source position due to macro expansion are merged.

[loop-copy.walz](../examples/optimization/loop-copy.walz) reads the first byte of a newly sliced bytes value on each iteration.
[loop-indexed.walz](../examples/optimization/loop-indexed.walz) reads the same immutable data directly.
With ordinary `-O3` in Binaryen 133, the first retains `array.copy`, while the second eliminates it.
Optimized total size changes from 253→214 bytes. Both are also checked to return 0 for empty data with zero iterations and trap with one iteration.
Inputs, diagnostics, and artifacts are saved in the [performance-warning verification record](../benchmarks/wasm-opt/20261110-advice-v1/report.md).
This is an explicit source rewrite example, not a determination that arbitrary copies can safely be removed.

These warnings are neither Binaryen's own optimization-failure notifications nor measured hotspots.
A copy may be a necessary snapshot, or the host may require object identity.
Decide whether to apply changes after checking loop frequency, boundary traps, mutation observations, data retention, and the host ABI.
Use warnings as a starting point for inspecting actual optimization results and profiles.

`--json` adds `performance` to the existing `walz-check-v1` format.
`diagnostics` remains for errors; `performance.warnings` contains warnings in the existing Diagnostic format, kept separate.
`performance.schema` is `walz-performance-v1`; `profile` is the evidence profile `binaryen-133-open-world`.
This does not detect an installed optimizer or invoke external executables.
`coverage: "opaque_wat"` means expression-level analysis of standard WAT is unavailable, rather than meaning no improvement candidates exist.
Without the flag, `check --json` retains its existing format.

The Rust API is `performance::check_with_policy(source, policy)`.
Use `performance::analyze(source, module)` to reuse checked IR.
Call `Report::located(&source)` to map positions back to multiple loaded files.
Analysis does not modify input IR or depend on output targets, so tasks targeting Core＋JSPI or Component can be checked without requiring Core output.

Enable warnings in LSP with `initializationOptions: {"performanceWarnings": true}` or
`settings: {"walz": {"performanceWarnings": true}}` in `workspace/didChangeConfiguration`.
Severity is 2 (Warning). Diagnostics show original-file positions, UTF-16 ranges, and related helper positions.
Results are reused within the same revision; disabling, editing, or closing clears stale warnings.
LSP continues to diagnose from typed snapshots; use CLI check/build for law proofs.

<a id="再実行"></a>

## Rerunning

Explicitly specify an executable whose `wasm-opt --version` is 133. Investigations using other versions need separately versioned tasks and records.

```sh
just test-wasm-opt /path/to/binaryen-133/bin/wasm-opt
just test-performance-warnings
just wasm-opt-capture new-run /path/to/binaryen-133/bin/wasm-opt
just wasm-opt-compare \
  benchmarks/wasm-opt/20261110-v1/before \
  benchmarks/wasm-opt/20261110-v1/after new-comparison
```

capture saves inputs, WAT, Wasm, optimized instructions and metrics,
compiler-implementation/input SHA-256 hashes, executed commands, and raw measurements under `target/wasm-opt/<run>/`. Existing runs are not overwritten.
compare checks matching inputs and profiles on both sides, saved input/Wasm hashes, and every case's values,
then measures in the same process, alternating before/after order.

Ordinary Rust tests cover 8–64 cases, reordered arms, nesting, one-time evaluation of side effects, branches, and owned values.
`test-wasm-opt` uses 133 to check optimized values, invalid tags/subtypes, nullary cases, and unused division traps
in Wasmtime and V8. Separate Core/Component ABI checks and Playground E2E tests are also present.
Performance-warning checks cover positive/excluded cases, import/macro positions, optional JSON/LSP enablement,
and the copy-presence differences above and host-observable GC-reference identity after Binaryen.
