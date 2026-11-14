# Watz clause keywords (2026-11-10)

Watz match clauses now use `case` and `default` without `@`. Lowering retains canonical Walz's `@case`/`@default` forms and their checks. Other annotations, including `@export` and `@effect`, are unchanged.

Reference distributions use stock-ledger v24, diagnostics v13, struct-readability v13, and macro/usability input revision 9. Earlier distributions and recorded results remain unchanged.

---

# Direct Watz control expressions (2026-11-10)

Optional Watz printing now represents match and if directly, including `@case`/`@default`, `else if`, and explicit result types. It lowers to the same Walz source forms and retains their checks. Unsupported native shapes keep structural spelling. Canonical Walz remains the primary source and default printed form.

Reference distributions use stock-ledger v23, diagnostics v12, struct-readability v12, and macro/usability input revision 8. Previous distributions and recorded results retain their bytes.

---

# Optional Watz printing (2026-11-10)

Canonical Walz S-expressions remain the primary source and default `walz print` format. `walz print --format watz FILE` selects an optional view using `func` and `@export("name")`. `walz print FILE.watz` prints it back as Walz. The former `review` command is replaced by this print option. Watz keeps the same types, effects, ownership, and Wasm semantics.

Reference distributions use stock-ledger v22, diagnostics v11, struct-readability v11, and macro/usability input revision 7. Earlier distributions and recorded results remain unchanged.

---

# Review notation `.watz` (2026-11-10)

The TypeScript-style review notation uses `.watz`, an optional `module { ... }` wrapper, and immutable `let` bindings. The printer omits the wrapper. Canonical `.walz` source and Wasm semantics are unchanged.

Reference distributions now use stock-ledger v21, diagnostics v10, struct-readability v10, and macro/usability input revision 6. Earlier distributions and recorded results remain unchanged.

---

# Match clauses and review notation (2026-11-10)

Match arms use `@case`, with an optional final `@default` for remaining shared-payload variants. Standard Component WAT `case` declarations retain their spelling. The optional `.walz.ts` review frontend maps source forms reversibly to S-expressions and uses the existing checks/runtime.

Frontend inputs/references use v7, stock-ledger v20, diagnostics v9, struct-readability v9, and macro/usability harness input revision 5. Earlier inputs and recorded results retain their bytes; new evaluations require fresh run IDs.

---

# Walz `@` extensions (2026-11-10)

Walz extension entry points now use `@`: `@use`, `@let`, `@match`, `@effect`, `@macro`, and `@$name`. The same prefix applies to extension value types, annotations, named construction, and generic instantiation. Percent-prefixed extensions are not source aliases.

Known Walz `@` names are reserved. They always enter checking and cannot be discarded by a WAT parser as unknown annotations. Other WAT annotations and their complete payloads remain opaque. WIT identifier escapes, URI percent encoding, and arithmetic remainder operators still use `%`.

Frontend inputs/references use v6, stock-ledger evaluations v19, diagnostic evaluations v8, struct-readability packets v8, and macro/usability harness input revision 4. Previous inputs and recorded results remain unchanged; use fresh run IDs.

---

# Walz naming (2026-11-10)

The current language name is **Walz**. The records below retain the names and paths used when those measurements were taken.

| Surface | Current name |
|---|---|
| CLI / Cargo package | `walz` |
| Application source | `.walz` |
| Tests / benchmarks | `*.test.walz` / `*_bench.walz` |
| Built-in package / implementation | `walz:core` / `walz/core/` |
| Host interfaces | `walz:host/...` |
| Browser compiler | `walz-web` |
| JavaScript package | `@mizchi/walz-runtime` |
| CLI override / tool environment variables | `WALZ` / `WALZ_*` |
| Local agent skill | `.agents/skills/walz/` |

CLI JSON schemas, LSP extension methods, generated names, and runtime ABI markers also use `walz`. Source syntax and Wasm semantics are unchanged. WLT compatibility aliases and a migration command are not provided.

Recorded artifacts and frozen inputs retain their original bytes. Frontend benchmarks use new v5 fixtures; evaluation suites and harness input revisions identify the renamed inputs separately. Historical WAT reference tests normalize generated identifiers and host namespaces explicitly. New runs must use fresh output directories.

---

# WLT への移行

このリポジトリは Wasmtime の作業ディレクトリにあった `examples/gcwat` の試作を取り出したものです。
Wasmtime 自体の Git 履歴やソースは取り込んでいません。最初のコミットに改名前の試作を保存し、その後で改名しています。
現在のWLTのライセンスは[Apache License 2.0](LICENSE)です。

| 対象 | 旧名 | 現行名 |
| --- | --- | --- |
| 言語・リポジトリ | gcwat / examples/gcwat | WLT / mizchi/wlt-lang |
| Rust crate・実行ファイル | gcwat | wlt |
| 拡張ソース・探索対象のテスト | .gwat / .test.gwat | .wlt / .test.wlt |
| WIT namespace | gcwat:* | wlt:* |
| private Core ABI marker | gcwat-core-js-v2 | wlt-core-js-v2 |
| JS の runtime package | gcwat-runtime | @mizchi/wlt-runtime |
| backend 選択・stress 検査 | GCWAT_BACKEND / GCWAT_WORKFLOW_STRESS | WLT_BACKEND / WLT_WORKFLOW_STRESS |

CLI は `cargo install --locked --path .` でインストールし、`wlt check file.wlt`、`wlt run file.wlt`、`wlt test directory` として使えます。
`just` の既定パスとサンプルも `.wlt` に変更しました。整形タスク名は `fmt-wlt` / `check-wlt-format` です。
標準 WAT の `.wat` は変更しません。入力ファイルの拡張子を強制する変更もしていません。

WIT import と private ABI の名前も変更したため、旧名で生成した Component / Core＋JSPI の成果物は再ビルドしてください。
既存の外部コードは import 名・環境変数・生成コードの設定を現行名に合わせます。

`docs/grammar-eval/`、`docs/readability/`、`evals/reports/` と workflow の `validation.json`・`tuning.json`・評価／計測レポートは過去の記録です。
入力とハッシュの対応を保つため、旧名・旧拡張子・当時のパスを残しています。これらは現行版を再評価した結果ではありません。
当時の入力を再実行する場合は改名前の初期コミットを別の checkout に取り出してください。
現行の評価 suite は版を 2 に上げており、`just eval-prepare ID` / `just eval-diagnostics-prepare ID` で新しい入力を固定できます。

`target/`、`node_modules/`、`evals/runs/`、ブラウザの検査出力は公開するソースに含めません。
既存のローカル成果物に記録された旧ディレクトリは書き換えず、新しい計測は新しい capture/run に保存します。

Rust 1.97 以降、just、Node.js 24 以降、pnpm を使います。実行検査には Wasmtime CLI、WIT 検査には wasm-tools、証明検査には elan と Lean v4.34.1 が必要です。
Wasmtime の checkout への path dependency はありません。依存は Cargo.lock と pnpm-lock.yaml に固定しています。

## 移行時の検査

2026-11-05、独立した `wlt-lang` ディレクトリで次を確認しました。

- `just check`：整形・全 target の clippy。
- `just test`：Rust の198件。
- `just test-language`：`.test.wlt` の32件。
- `node --expose-gc --test --test-force-exit runtime/*.test.mjs`：43件。Core / Component の取消しと GC 到達性を含む。
- `just test-e2e`：Chromium の24件。
- `just test-evals`：評価ハーネスの11件。
- `just proof-check` と `just wit-check`：Lean の4モデルと WIT の往復検査。
- `pnpm install --frozen-lockfile`：固定した依存のインストール。
- `just build-workflow` → `just workflow`：JSON Lines をパースし、全レコードを `expected.json` と照合。

CLI の回帰検査は一時ディレクトリから `wlt test .` を実行し、改名後のテスト探索が checkout の場所に依存しないことも確かめます。

## Explicit Component connections (2026-11-14)

Component `@bind` is now `@connect`; the WIT generator command is
`walz wit-bindgen`. Loop `(bind ...)`, macro binders and `--bindings` retain their
existing contracts. There are no compatibility aliases for the removed names.

The maintained `test-wasm-use` harness now selects binary constructors and
instantiates them explicitly, using native Component async/lifetime semantics.
Its Chromium case runs with the composition server. It no longer measures the
retired implicit per-path instance or SDK queued-cancellation behavior. Recorded
evaluation inputs and benchmark captures remain unchanged.

The maintained bootstrap frontend correctness corpus is now v2. It adds
`@lib` source libraries and rejects retired Component spellings/implicit binary
selection before host dependency I/O. Component code generation remains outside
the bootstrap subset; source diagnostics 219–223 describe this boundary. Frozen
compiler captures and benchmark corpora are unchanged.

## Bootstrap frontend parity corpus v3

The maintained correctness corpus adds function/type aliases and reexports,
scalar-class bounds and named/curly struct construction. The old unsupported
Add-constraint case now uses an unknown class, since Add is implemented. The
structural parser uses kind 6 for curly field groups; strings stay opaque data.
`just test-selfhost-parity` compares shared-syntax execution and rejection with
Rust. Native-index bootstrap regressions remain a separate gate. Frozen
benchmarks and their source/probe hashes are unchanged.


## Bootstrap frontend expansion corpus v4

The maintained correctness corpus adds bounded large-definition caches, unit
returns and long logical chains. It checks 300 distinct functions, 1,100 calls
reusing a specialization, rejection past 1,024 cached instances, valid empty
return, invalid return operands, and type checking after return. Logical-chain
cases verify short-circuit traps, source-order side effects and skipped type
errors. Shared-syntax cases also run against Rust; finite-resource failures stay
outside Rust acceptance parity. Cache and charged-work limits are now 1,024 and
1,048,576; structural parser, traversal and independent IR limits remain bounded.
Frozen benchmark captures, source inputs and reference hashes are unchanged.


## Bootstrap frontend profiling v1

The separate profiler entry adds request-local counters and immutable ABI
snapshots without changing the five production portable operations. Normal
entry points allocate no counter arrays. Concrete function instances cache
resolved signatures and return types; rejected computations are not published.
The new profile/comparison schemas record input, compiler/adapter and recorder
hashes. Timing comparisons require successful identical Wasm; failed outcomes
are recorded without timing ratios. The v4 correctness inputs and frozen scalar
benchmark captures are unchanged. See [profiling](docs/selfhost-profiling.md).


## Bootstrap frontend byte/nullability corpus v5

The maintained correctness corpus adds integer-array byte snapshots and typed
initializer regressions. i16/i32 sources are checked as unsigned octets only
within the selected range; existing i8 lowering remains in use. Invalid ranges
and values trap; source values, ranges, mutable-read effects and opaque boundaries
remain checked. Nullable values cannot be implicitly assigned to non-null let,
loop or recur bindings. Loop initialization keeps a typed temporary without
adding control frames. Rust's typed path requires named branches, so the native
numeric-depth regression is separate from identical-input parity. Shared cases
cover 56 successful inputs (including traps) and 27 rejected inputs. Node also
compiles the maintained `walz/core/bytes.walz` concat implementation. The embedded
bootstrap resource is regenerated from its source. Frozen measurements and their
input hashes are unchanged; v1 profiling measurements retain their pre-v5 inputs.


## Bootstrap layout/ordering corpus v6 and profiling v2

The correctness corpus adds repeated inferred field access, >128 GC types,
large explicit recursion groups, parent-cycle/group-boundary rejection and
numeric type-index preservation. Independent SCC tests compare reachability
partitions and dependency order, including 1,024-node chains/rings and edge
budget rejection. The new pass uses sparse edges and iterative Tarjan instead
of an all-pairs transitive closure. It admits 1,024 types and 65,536 edges.
Concrete layouts/field types are cached without bypassing access checks. The
aggregate cache permits 3,072 function/heap/alias instances, while each emitted
function/type pool remains bounded at 1,024. The previous 1,100-function case
now exercises the emitted pool; a new alias-heavy input tests the aggregate cap.
Profiler v2 adds layout/field counters, partial rendered-byte counts and distinct
edge/render limits. Retained v1/v5 input and measurement captures are unchanged.
A fresh full-compiler probe passes expansion/ordering and stops at the unchanged
1 MiB intermediate-WAT rendering cap. Shared Rust/bootstrap parity now covers
62 successful and 30 rejected inputs; native index/branch and resource-only
regressions remain separate. See [ordering](docs/selfhost-ordering.md).

## Bootstrap direct-syntax corpus v7 and profiling v3

Compilation and macro-query validation now assemble bounded postorder syntax
directly, retaining decoded strings and a separate original-source origin map.
The same independent checker still produces checked modules. Raw parsing and
debug rendering keep their existing text limits. The generated arena has
separate bounds of 262,144 nodes, depth 128 and 8 MiB of logical storage.

The v7 correctness corpus adds large decoded export strings, the same case with
macro query validation, and a rejected return mismatch. Shared Rust/bootstrap
checks now cover 64 successful and 31 rejected inputs. A separate unchanged-input
comparison retains binaries/diagnostics for 241 v6 frontend cases. Frozen scalar
benchmark inputs, source hashes and old recordings have not been rewritten.
Profiler v3 adds syntax counters and phase meanings; readers distinguish retained
v1/v2 artifacts. A fresh full-source probe reaches the generated-node budget
after expansion, ordering and lowering. It is not yet full self-hosting.
See [syntax assembly](docs/selfhost-syntax.md).

## Bootstrap tuning corpus v8 and profiling v4

The correctness corpus adds nominal list specialization stress; Rust/bootstrap
checks cover 65 positive and 31 negative inputs. Source-aware short-atom sharing
retains exact file/range/spelling identity, with bounded logical visits. Indexed
arenas are reused only after independent structure and position validation.
Wrong hints are normalized and unsupported contracts still fail the checker.

Profiler v4 adds logical syntax visits and atom hits, retaining reader support
for old schemas. The separate tuning-input schema v1 captures fixed subjects;
new output directories preserve each round's hashes, raw samples and counters.
The frozen compiler subject now reaches unsupported ABI metadata validation
rather than the syntax-node cap. Failed probes still have no timing ratio.
Earlier inputs and recordings remain unchanged. See [the tuning loop](docs/selfhost-tuning.md).

## Bootstrap checked-ABI corpus v9

The maintained frontend corpus adds checked closed record/tuple `@abi.type`
views. It validates portable fields and trusted core identities, memoizes bounded
shape/layout facts, preserves opaque ownership and source ranges, and removes
only certified metadata before independent instruction checking. Unsupported
views remain errors (225); ABI limits use 226. The Rust frontend also rejects
foreign annotations on opaque representations before inspecting the view.

Shared Rust/bootstrap checks now cover 76 successful and 64 rejected inputs.
The full-source probe passes ABI metadata and reaches independent GC type
validation (118) in `frontend/modules.walz`; it is not yet self-compiling.
Profiler v4 and frozen benchmark inputs remain unchanged. This does not add
WIT generation or host adapters. See [the ABI subset](docs/selfhost-abi.md).

## Bootstrap self-compilation corpus v10

The bounded frontend fixes empty result vectors, statement/condition contexts,
bare variable conditions, numeric literals distinct from explicit local indices,
and bitwise/shift/string-constructor inference. Bare string literals still need
an expected string type. Unsupported result vectors remain errors.
Statement blocks in the parser/checker/stack are aligned with Wasm typing rather
than relaxing independent validation. The Rust typed frontend accepts empty
function/control result vectors too.

The current compiler self-compiles into Core Wasm. Two successive self-compiled
stages and their portable outputs must be byte-identical. The maintained corpus
has 184 positive and 148 negative stage cases; shared Rust/bootstrap checks cover
107 positive and 68 negative cases. WAT numeric-local, labelled-if and branch
sequence cases are validated separately from Rust's typed `@use` path.
The Core GC bridge is for verification and
does not add WIT or general Component/JSPI adapter generation. The Rust compiler
remains the main implementation with broader coverage.

The new `walz-selfhost-stages-v1` capture retains source bytes/labels, probes,
artifact hashes and diagnostics. Profiler v4 and earlier benchmark/evaluation
captures remain unchanged; successful full-compiler timings use a fresh input
snapshot. See [stage verification](docs/selfhost-stages.md).

## Specialization inventory and opt-in code sharing

`inventoryProject` is a separate checked entry; existing profile v4 counters and
the portable five-operation ABI retain their contracts. Inventory captures v1
join guest declaration/argument origins to independently validated function bytes.
The code-sharing probe v2 adds a Binaryen 133 roundtrip baseline and snapshots
its recorder sources; the earlier v1 probe remains unchanged.

Measured ordinary helper duplication is factored in the parser/checker. Exact
postprocessing is opt-in, retains public/reference identity and GC types, and
rejects unknown custom index metadata when rewriting. Source validation and
default compiler output policy are unchanged. See [the contracts and captured
sizes](docs/selfhost-sharing.md); these results do not establish a speedup or
justify a default dictionary runtime.

## Selectable self-built Playground compiler

The browser compiler worker lazily selects Rust or a self-built Core GC stage.
The ordinary Playground build now produces both artifacts and requires identical
stage 1/2 output for the self-built image. Fresh seed/source/stage evidence stays
under `target/playground-selfhost-build-*`; browser metadata verifies the deployed
image hash. This does not change the portable five-operation ABI or add generated
Canonical ABI adapters.

The Core stage bridge is browser-compatible and adds optional expanded-source
inspection. Existing stage artifacts retain compile/import compatibility. The
self-hosted response labels this view as expanded Walz and analysis as validation
only, without Rust fallback. Compiler changes cancel pending workers/results and
preserve source. Shared diagnostic text retains the existing CLI messages.

The current [Wasm-body size figures](docs/playground-size.md#current-selectable-compiler)
are a fresh build, not a replacement for the earlier browser-size capture.
