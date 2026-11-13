<a id="walz-の実装計画"></a>

# Walz implementation plan

[English](TODO.md) | [日本語](TODO.ja.md)

Preserve a small language specification, inclusion of WAT, Wasm GC, and ease of static analysis.
Checkmarks mean implementation and regression checks are complete; design alone does not qualify.

## Component composition

- [x] Separate `@lib`, standard Core modules and Component definitions; select constructors and instantiate explicitly.
- [x] Infer concrete portable function adapters with `@expose` / `@bind`; preserve type/effect/ownership/proof gates.
- [x] Generate `world.wit` with `build --wit` and validate existing contracts with `--wit-input`.
- [x] Inspect memory identities/aliases and make generated ABI-memory accesses explicit; validate Core externref FFI metadata.
- [x] Node/Chromium checks for values, async bindings, separate instances, normal buffer retirement and native failure isolation.
- [x] Infer imported resource interfaces and own/borrow identities with instance `@bind`; retain nominal aliases, monomorphized owned exports, canonical drop and scoped exception cleanup. Check Node/Chromium identity, close counts, buffers and host-object reachability. The explicit native WIT backend remains available.
- [x] Add explicit generic source Component constructor arguments, with normalized closed value types, existing constraints and checks, definition sharing, independent instances and concrete WIT. Verify Node/Chromium values, endpoint moves, failures and buffer retirement.
- [x] Add read-only `@component.type` projections of explicit bindings, preserving ABI scalars, named type ports, shared payload restrictions and resource ownership. Check static modules, constructor phase boundaries, native host-result retirement/rejection and Node/Chromium execution.
- [x] Add closed visible nominal constructor layouts and staged queries of preceding connections. Preserve opaque/privacy boundaries, phantom identity, ABI annotations, independent instances and resource identity through validated value export ascriptions; check WIT roundtrips, Node and Chromium.
- [ ] Define host-native disposal/cancellation APIs for supported hosts without freeing in-flight copies prematurely.

See [Component composition](docs/component-composition.md) for contracts and current limits.

## Self-hosting compiler parts

- [x] Source-backed S-expression parser in `walz/parser/`: immutable postorder
  syntax, UTF-8 ranges, comments, WAT string decoding and explicit resource limits.
- [x] Independent scalar checker in `walz/checker/`: names, inference, immutable
  raw IR, operand/control stacks, signatures, labels and opaque checked modules.
- [x] Unit/i32/i64 binary backend: mutable locals, block/loop, br/br_if, return,
  if/select, single-binding lexical let, calls, exports and canonical LEB128.
- [x] Core byte builder with copied snapshots; bootstrap file helper; native,
  Core/Component, Chromium, trap cleanup and differential compiler checks.
- [x] Check-only file command and differential raw-IR checks against wasmparser.
- [x] Frozen scalar compiler benchmark: Core/Component, optimized Rust API and
  Rust WAT/validator references; canonical output parity gates and raw samples.
- [ ] Add floats, SIMD, block parameters and multiple results.
- [x] GC type IR/checking/codegen: structs/arrays, packed fields, recursive groups,
  declared subtypes, nullable references, casts and bulk array fill/copy. Check
  source and raw IR against independent validators on Core/Component/Chromium.
  Matching follows declared type indices/parents; full recursive type equivalence
  remains future work and some valid Wasm programs are conservatively rejected.
- [x] Conservative pure checks for mutable GC allocation/access, including
  transitive direct calls. Immutable GC construction and reads remain pure.
- [ ] Add memories, imports, function/external references and full recursive
  type equivalence; support non-defaultable declared locals with initialization checks.
- [x] 1. Static modules: relative `@use`, selected type/function exports, opaque
  access boundaries, file identity/ranges and a reserved embedded `walz:core`.
- [x] 2. Fixed-form lowering: sequential multi-let, typed loop/tail recur with
  simultaneous updates, for and list/tuple for-each; preserve lazy evaluation.
- [x] 3. GC effect bounds: pure/reads/writes/allocates, full instruction unions
  and transitive direct-call checks, including unreachable instructions.
- [x] 4. Minimal bootstrap core: sealed bytes/string/list/tuple/Option/Result,
  checked UTF-8 and snapshots, exhaustive match and list/bytes builders.
- [x] 5. Bounded type/function monomorphization: aliases, explicit/inferred
  applications, shared/nonunit constraints, caching and recursive groups.
  Check all five stages on Core/Component/Chromium and a native project example.
- [x] Declarative expression macros: typed fragments, repetition, recursive calls,
  definition/caller hygiene, explicit/repeated/delegated binders, infer/selectors,
  scalar guards and selected imports/aliases/reexports. Independently validate
  query expressions without executing or emitting them; enforce expansion budgets.
- [ ] Port remaining core and ownership/proof/ABI validation. Keep unsupported high-level contracts rejected until checks exist.
- [ ] Compile the compiler with stage 1; compare stage 1/2 outputs and validation
  against the Rust bootstrap before describing the implementation as self-hosted.

See [the compiler parts guide](docs/selfhost.md) and [frontend scope/limits](docs/selfhost-frontend.md). The current backend intentionally
rejects the features it needs to compile itself; it does not replace the Rust CLI.

## SIMD library roadmap

Keep these as ordinary libraries with opaque storage and explicit effects.
Measure construction/ABI costs separately from resident queries, and compare
against strong JS and scalar Wasm controls before claiming SIMD gains.

- [x] Foundation: immutable dense bitmap and i32 columns, Boolean selection,
  exact i64 sums, portable query batches, GC/ABI/Chromium checks, and frozen measurements.
- [x] 1a: Dictionary-encoded string columns. Publish a unique dictionary and
  validated SIMD code column; support value-based selection, decoding, and
  explicit dictionary/code snapshots without retaining a mutable build index.
- [ ] 1b: Narrow u8/u16 columns for 16/8-lane comparison. Validate representable
  values and tail lanes rather than silently truncating codes.
- [ ] 1c: Nullable columns with a same-length validity bitmap. Define selection,
  aggregation, and decoding of missing values without an Option object per row.
- [ ] 2: Bit-sliced indexes for small-width integers/enums. Validate bit width,
  plane lengths, and padding; compare predicates without decoding every value.
- [ ] 3: Sorted integer sets with SIMD intersection/difference. Check ordering,
  uniqueness, logical tails, and sparse-vs-dense crossover points.
- [ ] 4: SoA rectangle/coordinate columns for batched hit tests and visibility.
  Define bounds, row alignment, and NaN/infinity policy; return selection bitmaps.
- [ ] 5: SIMD hex/Base64 codecs and byte classifiers. Specify invalid-input,
  length/padding, and block-boundary behavior; include one-shot ABI costs.
- [ ] 6: Immutable Swiss-table-style indexes. Keep key equality checks after
  control-byte matching; validate capacity, probe termination, and collisions.

See [SIMD collections](docs/simd-collections.md). Priorities 1–6 are candidates;
performance results for the existing foundation do not establish their speed.
The [dictionary-column API](docs/dictionary-column.md) uses i32 codes initially;
native, both stable JS backends, forced GC, invalid snapshots, and Chromium are checked.
Its IN/range row query now fuses selection, ordered row emission, and exact sum
without intermediate bitmaps. Optional immutable count/postings indexes remain
future work; the published column does not retain a construction index.

## Arquero-style table library

- [x] Initial scalar implementation in `walz/table/`: immutable columns and row
  selections, select/filter/derive/groupby/rollup/orderby/slice, inner/outer/semi/anti
  equijoins, scalar aggregates, checked expression/schema boundaries, JS/TS facade,
  and Core + JSPI / Component pipeline snapshots. Compare with official Arquero
  8.0.3; check native execution, GC, trap cleanup, TypeScript, and Chromium.
- [x] Capture numeric-filter Arquero benchmarks with fixed inputs, original/packed
  scalar/SIMD controls and a direct JS count scan. Separate resident queries,
  fresh ingestion + query, and query + materialization. Preserve both runs in
  `benchmarks/table/`; do not transfer specialized collection speed claims here.
- [ ] Extend table benchmarks to aggregation, joins, ordering and standalone
  output/ingestion across more input types and backends.
- [x] Profile captured table artifacts: twelve CPU/heap workloads, separate
  unprofiled phase timings, exact export-call counts, tested block-short-circuit
  model and a separate Binaryen 133 probe. See `benchmarks/table/profiles/20261113-r1/`.
- [x] Batch numeric/Boolean/nullish I/O and hoist output column lookup/row count.
  Use a bounded 640-KiB transfer buffer with checked borrows/finally cleanup;
  strings and big-endian hosts retain scalar I/O. Preserve scalar tags and snapshots.
- [x] Track identical base/order selections and filter once when the invariant holds.
- [x] Short-circuit total numeric pair masks (AND zero / OR full) before evaluating
  the other subtree, while preserving general scalar short-circuit/trap behavior.
- [x] Fuse same-column ordered-comparison pairs, reusing block loads and converting
  vector Boolean results to bitmasks once. Retain the generic plan and packed scalar control.
- [x] Allocate exact numeric row-ID capacity from private pair masks, avoiding the
  prefix allocation/copy while preserving the public list snapshot constructor.
- [x] Use immutable primary numeric/tag storage for bulk Core input, lazy
  canonical Cell views, direct numeric output and CSP-safe row materialization.
  Preserve snapshots and declare native accessor cache effects explicitly.
- [ ] Extend bulk column output beyond `array(name)` to a complete columnar
  result API. Strings/portable snapshots still use Cells; scalar evaluation can
  materialize them, and SIMD preparation retains an additional packed cache.
- [ ] Reduce region-analysis cost for complex table/canonical-view alias graphs.
  Dependency scheduling and exact substitution/set-union reuse preserve analysis
  facts, but large native programs can still spend minutes in region summaries.
- [ ] Reduce repeated numeric input validation/publication using a checked opaque
  builder. Retain immutable snapshots and all tag/Boolean checks. The complete
  I/O suite still trails Arquero; input validation/copying remains significant.
- [ ] Evaluate whole-plan specialization and constant hoisting beyond fused pairs.
- [ ] Keep scalar filter temporaries unboxed where safe; retain generic value
  semantics for derive/materialization and executed-branch traps.
- [ ] Offer optional `wasm-opt -O3` table postprocessing after full target/browser
  validation. Raw Core probe shows 1.04x/1.08x SIMD improvements; default is unchanged.
- [ ] Specialize physical columns and plan execution using existing SIMD/dictionary
  kernels. Preserve null/undefined/NaN distinctions, JS numeric conversion rules,
  exact string ordering, and floating-point accumulation behavior.
- [x] Integrate lazy f64x2 caches for numeric constant comparisons and Boolean
  filter trees, with strict/loose type masks, safe scalar fallback and matching
  boxed/packed scalar controls across native, Core + JSPI and Component APIs.
  Aggregates retain scalar f64 order; dictionary planning remains separate work.
- [ ] Extend compatibility: assign/concat/dedupe/except/intersect/union, fold/pivot/
  spread/unroll, impute/lookup/cross/sample, reduce/relocate/rename/unorder,
  selection helpers, parameters, custom functions, and window/additional aggregates.
- [ ] Add Arrow and file formats, plus bulk ingestion/materialization adapters.
- [ ] Profile compilation, engine instantiation, and execution separately for
  larger GC expression modules; the debug native workflow is currently costly.

See the [table guide](docs/table.md) for the current compatibility boundary and
`just test-table` for the reproducible differential/ownership checks.

## Core improvements from binary codecs (1 → 2 → 3 → 4 → 5)

- [x] 1: Fixed-width, explicit-endian byte reads and packed-array writes. Typed operations and `walz:core` bytes/view readers specify unsigned offsets, signed 16-bit extension, bit-preserving integer/float conversion, evaluation order, and complete-range checks before mutation. Both endians, unaligned accesses, logical view bounds, effects, opacity, and both JS backends are checked.
- [x] 2: Validate UTF-8 directly over an immutable bytes-view and return a string-view without a payload copy. Explicit materialization, backing-buffer retention, and strict UTF-8 semantics are documented and checked on every backend.
- [x] 3: Core helpers for checked size/offset addition, multiplication, and alignment. Wasm i32/i64 wrapping semantics are unchanged, failure is explicit, and no implicit numeric conversion is added.
- [x] 4: Carry validated immutable ranges through view accesses, bounded shared-helper inlining, and per-trip unsigned loop guards. A separate must analysis removes proved redundant guards; unknown cases and Wasm GC array bounds checks remain.
- [x] 5: Eliminate eligible shared tuple/Option/Result carriers through local matches/drops, branches, and bounded helper inlining, including internal returns. Preserve payload identity, evaluation order, explicit traps/exceptions, and ownership boundaries; keep public ABI representations.
- [ ] Follow-up: General multi-value specialization for large/recursive helpers and loop-carried value containers. Current consumer fusion intentionally retains unproved/escaping representations and bounds code duplication.
- [ ] Later: A consuming mutable-buffer freeze into immutable bytes/list. First establish exclusive backing-storage ownership; the current Own wrapper does not imply uniqueness. Measure retained capacity as well as copy savings.

Keep new surface syntax small. Prefer core APIs and explicit typed IR operations over codec-specific instructions. See the [FlatBuffers release plan](walz/flatbuffers/TODO.md) for ABI/runtime work.

Step 1 is documented in [fixed-width byte access](docs/byte-access.md) and reused by the FlatBuffers reader/builder. Run `just test-byte-access`; Chromium coverage is in `runtime/e2e/byte-access.spec.mjs`. Workspace tests, `just check`, both JS backends, and eight Chromium byte-access/FlatBuffers checks passed.

Step 2 adds `$string-view.from-bytes-view`, `$bytes-view.is-utf8` / `to-string`, and optional ranges for `@bytes.is-utf8` / `@string.from-bytes`. The shared validator never reads outside the selected range; invalid UTF-8 returns none/0, malformed ranges trap, and copying follows successful validation. FlatBuffers uses range decoding. See [views](docs/views.md#validate-a-byte-range-as-a-string-view). Re-run `just test-utf8-views` and `just test-views`; GC identity/reclamation, suspension/rejection/cancellation, strict-decoder comparisons on four JS modes, workspace tests, `just check`, 23 Node cases, and 20 Chromium cases passed.

Step 3 adds six ordinary core functions: `$i32.checked-add-u`, `$i32.checked-mul-u`, `$i32.checked-align-up-u`, and i64 counterparts, returning Option. Unsigned high-bit values and nonzero power-of-two alignments are supported; overflow/invalid alignment returns none. See [checked integers](docs/checked-integers.md) and [layout example](examples/checked-layout.walz). FlatBuffers checks its vector byte count once outside the element loop, retaining its application limit. Re-run `just test-checked-integers`. Wasmtime optimization on/off, four JS modes against unbounded BigInt, 681 workspace tests, 11 Node cases, `just check`, and eight focused Chromium cases passed. The first all-fixture browser startup timed out at 300 seconds; the focused run rebuilt the relevant fixtures independently.

Step 4 adds bounded helper inlining with fresh locals/loop labels and must facts over immutable snapshots, normal successful accesses, and unsigned guards. Validated subranges compose without wrapping; loop initializers are not invariants. Proven readers omit two explicit helper guards while retaining GC `array.get_u` checks; original branch-condition evaluation remains. Diagnostic facts do not authorize these rewrites. Signed/i64 inference, recursive/general interprocedural contracts, and unsupported control-flow boundaries remain checked. See [range optimization](docs/range-optimization.md) and the [scan example](examples/range-scan.walz). Re-run `just test-range-optimization`. Optimized/unoptimized raw Wasm comparisons include null traps, host effects, exceptions, mutable reloads, and nested loop call sites. All 686 workspace tests passed (three existing ignored), as did `just check`, `just proof-check`, 19 Node cases, and eight focused Chromium range/FlatBuffers cases rebuilt from current sources. Lean proves arithmetic rules rather than the compiler implementation. No benchmark speedup is claimed.

Step 5 uses consumer fusion and shared-helper inlining rather than changing Wasm function signatures. Eligible local carriers become scalar payload locals; multiple matches, unused values, nested checked helpers, unit/null discriminants, and returned tuple/Result pipelines are covered. This also fixes dropped payload effects/traps in the earlier immediate-Result `@default` rewrite. Unknown/escaping values, foreign subtype checks, producer try/label/loop scopes, identity-bearing enums, and owned/resource/borrow functions retain their protocols. Failed budgeted rewrites restore the original form. See [value optimization](docs/value-optimization.md) and the [pipeline example](examples/value-pipeline.walz); re-run `just test-value-optimization`. All 692 workspace tests passed (three existing ignored), plus `just check`, `just proof-check`, 35 Node cases, and 12 focused Chromium value/range/FlatBuffers cases. Raw Wasm optimization on/off, host traces, forged values, four JS modes against BigInt, payload rejection/cancellation, and ABI cleanup are checked. General multi-value lowering and measured speedups remain separate work; rewrite correctness is compiler TCB, not a new Lean proof claim.

## Angle application notation

- [x] Accept `$Name<T, U>` as shorthand for `@inst` in type/function/heap positions, including nested value types, generic closures, named construction, macro type arguments, and concrete exports. Preserve explicit reference wrappers, existing checks and specialization sharing, source locations, literal WAT names, and operators. Support formatting, Watz views, LSP recovery, and a Playground example.

See [the rules](docs/generics.md#angle-application-shorthand) and [the complete example](examples/angle-applications.walz). Re-run `just test-angle-applications`. Shorthand/explicit Wasm equality is checked with optimization on/off; namespace/import aliases, literal/quoted/suffixed WAT names, malformed/deep applications, bounds, error spans, and incomplete editor buffers are covered. All 701 workspace tests passed (three existing ignored), plus `just check`, `just proof-check`, documentation checks, five Node cases (four JS modes plus sample coverage), and two isolated Chromium Playground cases using a freshly built Wasm compiler. English/Japanese cheat sheets and specifications and the Walz skill reference are updated. The existing Playground process was preserved.

<a id="ユーザー定義構文1--2--3--4"></a>

## User-defined syntax (1 → 2 → 3 → 4)

- [x] 1: Retain syntax-node identity, expansion contexts, and input/output mappings. Track duplication, deletion, and definition origins.
- [x] 2: Declarative, hygienic expression macros with `@macro`, explicit `@use` / `@export`, repetition, and expansion limits.
- [x] 3: LSP signatures, definitions, completion, expansion views, and diagnostics. Do not incorrectly display types of discarded arguments or generated temporaries on the original source.
- [x] 4: Declare and check caller bindings and body scopes through `bind`. Use declaration information even while editing erroneous code.

Use `infer` to obtain ordinary inferred types and `when` to select rules by types and existing type classes. Generated type applications return to existing monomorphization without skipping type, effect, ownership, or race checks. See the [contract](docs/user-macros.md) and [executable example](examples/user-macros/main.walz).

<a id="マクロの実用性から追加した機能"></a>

## Features added for practical macro use

- [x] Infer ordinary generic-function type arguments at call boundaries. Check ambiguity, constraints, effects, ownership, and recursion with identical types.
- [x] Read-only type information and structural selectors in `walz-type-info-v1`. Do not expose opaque internal types.
- [x] Differential fuzzing of values, side effects, exceptions, and ownership across macros, handwritten expansions, and independent models, with reduction and replay preserving the failure category.
- [x] Show input duplication and disposal in JSON/LSP and `macro-expand --text`. Distinguish syntax occurrences from execution counts.

<a id="マクロ評価で残った4課題"></a>

## 4 remaining tasks from macro evaluation

- [x] 1: Recheck only the affected function after expansion. Instantiations changing the type environment recheck the whole module, retaining final whole-module contract checks. Check 256/512 functions under existing limits.
- [x] 2: Delegate explicit binding interfaces matching callees. Reject leakage into initializers and ambiguous rules.
- [x] 3: Bind repeated idents, order let-group expansion, and handle empty sequences, duplicates, initializers in caller scope, and multilevel delegation.
- [x] 4: Recover current imports and unsaved macro declarations even from incomplete buffers. Provide original-file definition positions and completion while leaving types and expansions unchecked.

[Follow-up with the same evaluation inputs](evals/reports/20261109-macro-usability-v1-r2/REPORT.md): all 20 required checks and the 4 previously unmet tasks passed.

<a id="マクロ評価-v2-の改善"></a>

## Macro evaluation v2 improvements

- [x] Resume checking within the same function from the expression. Do not retain inference's temporary locals; preserve whole-module rechecking on instantiation and final contract checks. Regression-check 512 expansions in one function and diagnostics for deep expansion.
- [x] Explain nonterminal repetition, whole-body forwarding, and caller scope for initializers in diagnostic help.
- [ ] Verify elimination of scalar expressions/locals while preserving traps, exceptions, and ownership.
- [ ] Evaluate recursive Result-binding macros as a separate task to determine whether additional syntax is needed.

[Comparison with the same inputs](evals/reports/20261109-macro-authoring-v2-r3/REPORT.md): all 11 required checks passed, as did 512 expansions in one function. The 3 remaining synthetic tasks remain limitations of the current specification.

<a id="非同期機能を実用へ接続する1--2--3--4"></a>

## Connecting async features to practical use (1 → 2 → 3 → 4)

- [x] Core＋JSPI test/bench, explicit hosts and bindings, per-case cleanup and deadlines.
- [x] WIT shared-value types and synchronous world export adapters. See below for native async/resource support.
- [x] Explicit Stream batch reads, sendMany reporting partial acceptance, file chunk transfers, and comparison benchmarks.
- [x] Deadlines and cancellation for public calls and host-side scopes, and owned timer futures. Also add deadlines for Walz child cancellation/join.

See [batch transfers](docs/stream-batches.md) and [scopes and deadlines](docs/scoped-tasks.md).

<a id="今回の追加1--3--5--4"></a>

## Additions in this round (1 → 3 → 5 → 4)

- [x] 1: Memoize type graphs and ABI layout/flatten/schema; add size limits and DAG regression checks.
- [x] 3: Per-child-task Walz cancellation and join-timeout. Check cleanup on both JS backends and in browsers.
- [x] 5: stdio LSP diagnostics/hover/definition. UTF-16, unsaved imports, cache limits, and request cancellation.
- [x] 4: Native WIT async worlds and own/borrow for imported resources. Check standard Canonical ABI with raw jco and Wasmtime 50.

[LSP](docs/lsp.md) and [native worlds](docs/native-worlds.md) describe execution requirements and unsupported scope. HTTP and WASI network bindings are separate work.

<a id="使い勝手の修正1--2--3--4--5"></a>

## Usability fixes (1 → 2 → 3 → 4 → 5)

- [x] 1: Exclude generated test/bench exports from ordinary JS/Component ABIs while preserving full-source checks and each runner's behavior.
- [x] 2: Display monomorphized type/function diagnostics using type arguments and in-scope aliases. Also bound diagnostic-display expansion.
- [x] 3: Recover current declaration syntax in LSP, distinguishing unchecked displays. Handle unfinished parentheses, moved definitions, lexical shadowing, and core-import completion.
- [x] 4: Synchronize explanations of individual cancellation, native own/borrow, and backend-specific stdout.
- [x] 5: String literals with determined expected types, and `$option.ok-or` / `$result.map-err` for shared/nonunit. Check evaluation order and GC identity.

The [post-fix evaluation](evals/reports/20261108-usability-fixes-v2/REPORT.md) records check results and limitations.

<a id="高水準の糖衣1--2--3"></a>

## High-level sugar (1 → 2 → 3)

- [x] 1: `@for-each`. Iterate lists of shareable nonunit elements using single names, tuples, or field patterns.
- [x] 2: `@let {:field $binding ...}`. Read required fields once from non-null shared structs and bind them to locals.
- [x] 3: `@struct.update`. Ordinary mutable-field updates evaluating the subject, old value, and update expressions once each.

See the [contract](docs/high-level-forms.md) and [executable example](examples/high-level.walz). Expand into existing IR and check types, effects, ownership, and opaque boundaries. Check LSP, optimization on/off, Core/Component/Chromium execution, and suspension/exception paths.

<a id="cli-から実用アプリケーションまで2--1--3--4--5"></a>

## From the CLI to practical applications (2 → 1 → 3 → 4 → 5)

Use Rust 1.99 as the implementation and CI baseline.

- [x] 2: `walz run` entry points, arguments, standard input/output, exit codes, and explicit host capabilities. Also wait for cleanup to complete after interruption.
- [x] 1: Bundle only required runtimes in `walz build --target core-js`, producing JS artifacts that need no checkout.
- [x] 3: WIT records/enums/flags/unsigned types, world selection, typed export stubs, and WASI 0.3.0 file value adapters.
- [x] 4: Share types with the Wasm validator, supporting numeric/SIMD memory instructions and typed macros in the same function. Keep unknown instructions and ownership boundaries closed.
- [x] 5: Shared-payload Future/Stream writers, backpressure/close/cancellation, and scoped spawn/join. Run on both JS backends and in Playground.

Supported scope and limits are documented in [CLI](docs/cli.md), [WIT](docs/wit-bindings.md), [WASI value adapters](docs/wasi-files.md), [validator](docs/wat-validator.md), and [writers/child tasks](docs/scoped-tasks.md).

<a id="直近の優先順位"></a>

## Immediate priorities

Address gaps identified in complex applications in the following order.

1. [x] Consolidate bytes/text search, splitting, checked integer parsing, and text/JSON builders in the standard library. Reuse them in log aggregation, inventory ledger, and workflow, comparing output and measurements.
2. [x] Expand differential checks between macro/explicit versions and Wasmtime/Node. Fuzz invalid sources and cancellation timing, including failing-input reduction and regression corpus storage.
3. [x] Generate WIT bindings for existing value types. Specify effects/capabilities through a separate trusted policy rather than inferring them from WIT.
4. [x] Add typed external resource handles and explicit close, checking cleanup on success, Result err, exceptions, and cancellation. Current own boxes have no OS-resource destructors.
5. [x] Simplify Results/Pairs matched immediately after construction as a small typed-IR optimization. Add checks preserving evaluation order, exceptions, and ownership, separating proved and unproved scope.
6. [x] Implement JSON values and parsing in the Walz library. Handle arrays, null, numeric lexemes, and Unicode escapes; assess duplication before deciding on generics.

The APIs and boundaries for 1 are in [walz/core/README.md](walz/core/README.md); before/after output comparisons, measurements, and generated sizes are saved in the [implementation record](docs/stdlib/20261105.md).
Added APIs for JSON values, parsing, arrays, null, and decimal/exponent lexemes. Numbers retain exact text. Binary-float conversion comes later. See [JSON values](docs/json-values.md).

<a id="型付きコアの小さな拡張1--2--3--4--5"></a>

## Small typed-core extensions (1 → 2 → 3 → 4 → 5)

- [x] Named struct construction. `@struct.new`, independent field-map/expected-type literal sugar, once-only source-order evaluation, and existing opaque/ownership/effect checks remain intact. See the [contract](docs/struct-construction.md).
- [x] Shorthand for local references and expected-type numeric literals, plus let `(type T)` annotations. Reject ambiguous widths and implicit moves/conversions; check monomorphization, static analysis, and both JS ABIs.
- [x] Add/Eq/Ord constraints and symbolic operators for basic types. Check fixed instances during monomorphization, resolving to existing Wasm instructions without dictionaries. See the [contract](docs/typeclasses.md).
- [x] Logical/Bitwise/Shift constraints. Short-circuiting `&&`/`||`, `&`/`|`/`^`, and `<<`/`>>`/`>>>`. Preserve right-hand static checks, ownership, waits, and Wasm shift rules.
- [x] Sub/Mul/Div constraints and `- * /`. Preserve Wasm wrapping, signed-division traps, and float division; reorganize json-value with shorthand.

- [x] 1: Standard scalar conversions, saturating conversions, rounding, sqrt, min/max, reinterpret, rotations, and sign extensions. Check signatures against the Wasm validator.
- [x] 2: `@result.do`. Expand into existing result.let, preserving once-only left-to-right evaluation and err short-circuiting.
- [x] 3: Scalar newtypes and explicit wrap/unwrap. Preserve nominal checks and opaque boundaries; erase runtime representations.
- [x] 4: Typed return and named block/br/br_if. Track ownership and escape; close only exited using scopes in reverse order.
- [x] 5: Combined owned values and exceptions. Check owned slots during argument evaluation, catch, iteration, and unhandled-exception paths with Rust and Lean.

See [numbers, Result, newtypes, and control flow](docs/core-expressions.md) for specifications and limits. Check with `just test-core-expressions`, both Node ABIs, and Chromium.

<a id="開発ツールと標準コレクション"></a>

## Development tools and standard collections

- [x] Make existing diagnostic contracts and exit codes machine-readable with `check --json`.
- [x] Query original-file lines and Unicode columns with `type-at`, returning type annotations, type aliases, and instantiation positions. See [inspection for external tools](docs/tooling.md).
- [x] Provide text-keyed `dict<T>` and immutable-list-building `list-builder<T>` as ordinary Walz libraries. Check shareable/nonunit constraints, opaque boundaries, and effects.
- [x] Check collisions, growth, updates, deletion, reuse, reference identity, and immutable snapshots in Wasmtime; compare Core＋JSPI/Component against independent Maps. See [collections](docs/collections.md).

<a id="リポジトリの独立"></a>

## Repository independence

- [x] Separate from the Wasmtime working directory, renaming the language/crate/CLI/extension to Walz / walz / .walz.
- [x] Publish `mizchi/walz-lang` and record checks in an independent environment.

<a id="1-文字列byteslist-の境界表現"></a>

## 1. Boundary representations for strings, bytes, and lists

- [x] Immutable bytes/text GC representations and Canonical ABI adapters for WIT list<u8>/string.
- [x] Immutable list<T> of shared elements and ABI adapters. Forbid owned elements and borrows. See [lists](docs/lists.md).
- [x] Strict UTF-8 validation, byte lengths, unsigned indices, and slices by starting position and length.
- [x] Checked bytes.from-array and the initial standard library for concatenation, integer conversion, and JSON strings.
- [x] bytes/text search and splitting at the first separator, and fragment-accumulating text/JSON object builders.
- [x] Zero-copy views of immutable bytes/strings. Check relative ranges, UTF-8 boundaries, opaque boundaries, and effects; verify GC sharing/collection and both ABIs. Standardize the type name as `$string`. See the [contract](docs/views.md).
- [ ] Unicode character counts, graphemes, normalization, and similar operations.

<a id="2-汎用-wit-binding-と実用-io"></a>

## 2. Generic WIT bindings and practical I/O

- [x] Generate import and export implementation-stub types from supported WIT value types.
- [x] Native export-instance adapters for synchronous shared-value worlds. Check original interface names, types, and function signatures.
- [x] Shared-payload async endpoints and imported-resource own/borrow ABIs for native worlds. Lists/records of owned elements and full WASI worlds are excluded.
- [x] Fixed text-I/O types, host effects, required capabilities and manifests, and explicit host connections.
- [x] Freestanding imports of supported WIT value types and explicit effect/capability policies. See [WIT bindings](docs/wit-bindings.md) for supported scope.
- [x] Connect Node standard input/output and files to text-I/O capabilities passing only values.
- [x] File value adapters pinned to WASI 0.3.0. Host-owned descriptors and explicit named grants.
- [ ] WASI native-resource worlds and network bindings.
- [x] A CLI that filters empty lines from UTF-8 input and releases resources even on mid-operation cancellation.

<a id="3-外部-resource-と終了処理"></a>

## 3. External resources and cleanup

- [x] Typed resource handles and synchronous destructors, distinct from existing own boxes.
- [x] Explicit close and using. Check every exit path: success, exceptions, and cancellation.
- [x] Combined owned values and try/throw. Check normal/exceptional/branch paths and owned values during evaluation, comparing with Lean control-flow certificates.
- [ ] Future/Stream delivery of owned payloads.
- [x] Borrows limited to required ABI boundaries. Assess the need for a general lifetime mechanism later.

<a id="4-モジュールと可視性"></a>

## 4. Modules and visibility

- [x] `@use` synchronous shared-value exports from closed, WIT-bearing Component `.wasm` files. Type/effect checks, selection/aliases/re-exports, the same instance, dependency-wide DCE, and static embedding.
- [ ] Async/Future/Stream/resource boundaries for Component dependencies. Define cancellation-exception, join, and endpoint-lifetime contracts before acceptance.
- [ ] Explicit connection of Component dependencies' host capabilities and calls from Core/Core＋JSPI. Currently Component output only.

- [x] namespace/public/use and namespaces, relative includes, cycle detection, and static linking into one Wasm module.
- [x] Private functions/types/tags and checks on public names.
- [x] Opaque declarations for public types, also applying to aliases and inferred references.
- [x] File-level named imports/exports and re-exports. Share definitions/types with the same canonical path, binding only selected names.
- [ ] Field-level visibility.
- [x] bytes/text standard libraries and hidden mutable internal representations for log aggregation.

<a id="5-利用者定義のジェネリクス"></a>

## 5. User-defined generics

- [x] Monomorphization parameterized by types and concrete closure types. Share explicit type.params and application forms across functions, types, aliases, and closures.
- [x] Keep instantiated types and callees closed. Check types, effects, ownership, and opaque boundaries before DCE.
- [x] Write map/fold and similar functions without duplication. walz/core/list.walz and examples/generic-lists.walz. See [monomorphization](docs/generics.md).
- [x] shared/nonunit/callable and effect limits through where. Check types, signatures, and inferred effects on instantiation, leaving no constraints in executable code.

<a id="6-解析精度と契約の拡張6163-の初版を実装"></a>

## 6. Analysis precision and contract extensions (initial 6.1–6.3 implemented)

- [x] 6.1 Read/write targets distinguishing arguments, captures, allocation sites, and unknown values.
- [x] 6.2 Escape analysis tracking aliases, storage destinations, calls, and returns.
- [x] 6.3 Retain branch-derived non-null, i32-range, and array-bound facts with expression positions.
- [ ] 6.4 Explicit loop invariants/decreases and proof obligations at entry/iteration.
- [ ] 6.5 Preconditions/postconditions and extended laws covering GC, arrays, and recursion.
- [ ] 6.6 Optimization passes using analysis evidence and checks of semantic preservation.
- [x] Remove unreachable functions/types/tags/host imports. Preserve full-source type/effect/ownership checks and align IDs and ABIs across Core/Component/profilers.
- [x] Wasmtime guest profiling, reproducible Core＋JSPI comparison measurements, and ABI-allocation/UTF-8-validation tuning.

Preserve existing effect limits and ownership checks. Do not reinterpret may information as must non-aliasing or purity.
Do not handle unknown code, recursion, loops, exceptions, or suspension through unchecked assumptions.
Record analysis improvements separately from soundness proofs of contract models/the compiler.

6.1–6.3 are implemented in `analyze` memory/facts and the Rust API. No non-aliasing between arguments is assumed;
allocations merge by site, and access paths widen to unknown at depth 3.
Numeric ranges use unsigned i32 bit intervals; loop-carried values widen to type limits.
i64 intervals, mutable-field value refinement, and interprocedural range contracts come later.
Existing effect contracts and Wasm output are unchanged; instruction elimination based on these facts belongs to 6.6.

<a id="7-writer-と言語内タスク"></a>

## 7. Writers and language-side tasks

- [x] Core＋JSPI output for JS. Generate codecs for used types, sharing existing cancellation and Stream cleanup.
- [x] Shared-payload pending Future/Stream writers and ownership protocols using existing resources.
- [x] Per-element acceptance/termination, backpressure, close, and cancellation.
- [x] Scoped spawn/join/drop. Check explicit captures, GC identity, sibling failure, parent cancellation, and cleanup.
- [x] Stream batch transfers and partial progress, deadlines/cancellation for public calls and host-side scopes.
- [x] Cancellation/deadlines for only a Walz child. Preserve parents and siblings while waiting for descendants and disposals to finish.
- [ ] select and cancellation tokens for arbitrary scopes.
- [x] Connect shared-payload futures/streams to standard native async ABIs. The Walz SDK child scheduler is a separate boundary.

<a id="8-任意の-wat-命令とマクロの混在"></a>

## 8. Mixing arbitrary WAT instructions and macros

- [x] Check `v128` and regular non-memory SIMD instructions in the typed core. Verify GC storage/captures/monomorphization and Core/Component/Chromium. Explicitly convert to scalars or similar values at ABI boundaries. See [SIMD](docs/simd.md).
- [x] Implement 4 bulk kernels informed by jsimd, comparing resident GC values and copy-inclusive calls against JS and reference Wasm. See [measurements](docs/simd-kernels.md).
- [x] Improve the Core＋JSPI numeric-list ABI identified by SIMD measurements using matching TypedArray bulk copies. Preserve value checks, cleanup, and Array paths, alternating before/after measurements with identical Wasm.
- [ ] Reduce intermediate GC copies before packing and increase packed-data reuse. Preserve type/value checks and exception cleanup; remeasure resident data separately from one-shot calls.
- [x] Mix numeric/SIMD linear-memory instructions with typed macros.
- [x] Share instruction types/results with the Wasm validator's operand stack.
- [x] Reject ownership bypasses through a closed instruction list, conservative effect/escape analysis, and numeric-only boundaries.
- [ ] Generalize to arbitrary stack intervals, multiple values, and table/ref instructions. Do not automatically accept unknown opcodes.

Check bytes/text and modules in Rust/Node/Chromium. See SPEC.md for syntax and ABI details.
The UTF-8 validator, file expansion, name resolution, and adapters are in the TCB, without Lean proofs of semantic preservation yet.

<a id="ログ集計からの改善1--2--3--4--6"></a>

## Improvements from log aggregation (1 → 2 → 3 → 4 → 6)

- [x] 1: GC-array copying and string libraries. Check aliases, UTF-8, and integer boundaries.
- [x] 2: result.let / stream.fold-result. Post-expansion type/ownership/effect checks and cleanup on EOF/err/cancellation.
- [x] 3: Opaque types and level enums. Aggregation state mutable only from within.
- [x] 4: Comment-preserving fmt/fmt-check and diagnostics for opening-parenthesis positions and application errors.
- [x] 6: Alternating comparisons varying line lengths, invalid-input rates, and chunk sizes. JSON output on 1 line, line-enum integration, and direct aggregation-state references.

Step 5 in this sequence (monomorphized generics) implements explicit type arguments and post-instantiation checks. 6.6 implements simplification of immediate matches on shared Results/Pairs; general analysis-dependent optimizations come later.

<a id="改善後の実用例での確認"></a>

## Verification with practical examples after improvements

- [x] An inventory ledger handling Unicode item names, with a concrete GC hash table and dynamic JSON Lines.
- [x] Compare handwritten/macro results, I/O failures, and cleanup after cancellation; execute in Core/Component/Chromium.
- [x] Record main source volume, expanded IR, generated size, and alternating execution times.
- [x] Library support for bytes/text search, splitting, and integer parsing.
- [x] Simplify shared Results/Pairs matched immediately afterward. Treat this as a 6.6 task with semantic-preservation checks.

<a id="共有メモリの検証"></a>

## Shared-memory verification

- [x] Verification mode for fixed atomic cells, host contracts, and an independent Lean trace kernel.
- [x] Reject raw addresses, partial overlap, and non-atomic instructions; compare against logical lost updates with load/store.
- [x] Readonly regions immutable after publication, loads checking logical bounds, combined private memory, and an independent Lean trace kernel. Explicitly state host initialization/immutability assumptions.
- [ ] Exclusive-region partitioning through Owned<R>, moves between Workers, and join.
- [ ] Protocol invariants and linearizability for locks/channels. Treat separately from data-race prevention.

<a id="簡潔索引と汎用マルチスレッドホスト"></a>

## Succinct indexes and a generic multithreaded host

- [x] Immutable BitVector rank/select and u8 Wavelet Matrix rangeFreq/quantile. Compare empty/boundary/missing cases with an independent oracle and JS using the same scheme.
- [x] Select scalar for standalone rank and SIMD within Wavelet through alternating scalar/SIMD comparisons. Save 1/2/4 Workers, actual jsimd public APIs, and communication-inclusive batch measurements as new runs.
- [x] Separate queues, per-job cancellation, join, and shutdown into a generic Node/browser WorkerHost. Keep index layouts in drivers. Run another async Walz program through Core＋JSPI and Component.
- [x] Copy before publication, unexposed shared memory, and wait for all readers to finish. Check individual cancellation bits, retries after shutdown failure, and Chromium.
- [ ] Reduce shared-memory capacity for small indexes. Currently fixed at 16 MiB; byteLength is used size.
- [ ] Measure cold-start distributions and p95 including index construction/Worker startup, and break-even points in real applications.

See [Worker host](docs/worker-host.md), [readonly regions](docs/readonly-regions.md), and [indexes and measurements](docs/succinct.md). Guarantees exclude race freedom of arbitrary JS drivers/the entire host, static Builder freeze, and GC-reference transfers between threads.
