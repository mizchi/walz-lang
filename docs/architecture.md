# Implementation

```text
ファイル解決・walz:core の静的展開 → S 式の正規化・静的な名前解決
  → マクロ宣言・汎用テンプレートの収集
  → 具体化・マクロ展開・型検査（必要な型がそろうまで再開）
  → 型付き IR → 全ソースの解析・契約検査
              → 到達性による宣言の除去 → WAT → Wasm
```

- `src/ir.rs`: Types, local/function/loop IDs, and a closed expression enum. The shared contract for analysis and emission.
- `src/sexpr.rs`: S-expressions with locations, snapshot-local node IDs, and expansion contexts. Supports `;;` and nested `(; ;)` comments.
- `src/source.rs`: Relative imports/includes, file sharing and cycle detection, and mapping diagnostics back to original files.
- [src/standard.rs](../src/standard.rs): Selects bundled `walz:core` type aliases and standard functions, expanding them into ordinary static modules. Does not search files or perform runtime initialization.
- `build.rs` / `src/standard_catalogue.rs`: Indexes public standard API names and type-alias locations at build time. `standard.rs` reads only the required implementations and shares immutable syntax. See [Rust-side measurements](frontend-performance.md).
- `crates/walz-web` / `playground/`: Exposes the shared compiler through wasm-bindgen and runs compilation and guest execution in separate Web Workers. See [browser builds](playground.md).
- [src/syntax.rs](../src/syntax.rs): Normalizes entry points beginning with `@` and expands multiple-binding `@let` into existing nested bindings.
- `src/modules.rs`: Separates definitions and import aliases in a kinded symbol table, processing collection, import resolution, and declaration expansion in that order.
- `src/macros.rs` / `src/macros/`: Declarative expression macros, hygienic bindings, delegation, structured type information, input-to-expansion mappings, and expansion budgets. Returns existing expression syntax rather than generating checked IR.
- `src/generics.rs` / `src/generics/inference.rs`: Instantiation and reuse for explicit or inferred type arguments, type equalities, recursion/expansion limits, and diagnostics. Leaves no generic declarations in executable IR.
- `src/bounds.rs`: Checks where-clause shared/nonunit/callable constraints and effect bounds against concrete types, signatures, and inferred effects.
- `src/type_syntax.rs`: A shared visitor distinguishing value-type positions from nominal function/heap-type references.
- `src/link.rs`: Selects reachable declarations from all checked IR and reassigns IDs for emission.
- `src/buffers.rs`: Private GC storage for bytes/text, UTF-8 validation, and value operations.
- `src/frontend.rs`: Checks types, names, captures, mutability, match exhaustiveness, and tail positions.
- `src/frontend/generic_calls.rs` / `macro_types.rs`: Type inference for ordinary generic calls and read-only type queries for macros.
- `src/frontend/macro_expansion.rs` / `lexical.rs`: Resumes checking from expressions after macro expansion and discards provisional local IDs. Separates let/block/if checking from the large instruction dispatcher.
- `src/typing.rs`: Listings of declared/inferred types and type queries by source location.
- `src/inspect.rs`: A read-only API matching checked IR against original type annotations and instantiation sites using original-file lines and columns.
- `src/cli_inspection.rs`: CLI input/output for JSON diagnostics and type-at. Separates checking logic from display metadata.
- `src/native.rs`: Function signatures for standard WAT outside the typed core.
- [src/native_macros.rs](../src/native_macros.rs): Mixes WAT outside the core with syntax-only macros. Validates the whole module with a Wasm validator and marks analysis opaque.
- `src/runner.rs`: Test discovery, isolated execution, timeouts, and structured results.
- `src/benchmark.rs`: Checks bench result types, links each case, and performs measurement and statistics. `src/harness.rs` provides discovery and process management shared with tests.
- `runtime/bench-runner.mjs`: Warm-up, batch calibration, and monotonic-clock measurement for a persistent Wasm instance.
- `src/analysis.rs`: Effect and call-graph analysis. Traverses typed IR without reparsing WAT strings.
- `src/regions.rs`: Target-specific reads/writes, reference flow, allocation sites, and escape analysis.
- `src/flow.rs`: Analysis of non-nullness, i32 intervals, and array bounds along normal control flow.
- `src/effects.rs`: Source-observable effects, exceptions escaping functions, and declared effect-bound checks.
- `src/ownership.rs`: Ownership propagation, explicit moves, and ownership-set contracts at branches, loops, and exits.
- `src/proof.rs`: Generates BitVec law proof obligations from typed IR.
- `src/verified.rs`: Emits the same IR that passed Lean checking to Wasm.
- `proof/Ownership.lean`: A small ownership checker, declarative rules, and proofs of checker soundness and rejection of double consumption.
- `proof/Delivery.lean`: Models and proofs for operations, ordering, termination, and close on delivered Futures/Streams.
- `src/wit.rs`: Generates WIT contracts from public function types.
- `src/bindings.rs`: Fixed types and effects for event-loop imports.
- `src/component.rs`: Builds a Component and manifest from checked types and core Wasm.
- `src/component/abi.rs`: Portable types and Canonical ABI layouts, flattening, and signatures.
- `src/component/values.rs`: Converts GC values and ABI values; copies bytes/text and compound values and releases buffers.
- `src/component/endpoints.rs`: Transfer, read, and close for readable handles.
- `src/component/functions.rs`: Core import/export boundaries and task cleanup.
- `src/component/core_js.rs`: Generates JS-specific core Wasm and bindings from checked IR.
- `src/component/core_js/values.rs`: Generates JS codecs for the required concrete types.
- `src/component-memory.wat`: Allocation and release of conversion buffers.
- `runtime/text-io.mjs` / `node-io.mjs` / `text-cli.mjs`: Value-only line I/O and Node handles/CLI.
- `runtime/core.mjs` / `build-core.mjs`: JS-specific Core/JSPI host and builds. Isolates state per instance.
- `runtime/`: JSPI generation for Components, a shared Promise/AsyncIterable host, task requests/completion acknowledgement/release, and Node/browser execution tests.
- `proof/Tasks.lean`: State models for cancellation requests and completion acknowledgement, with proofs rejecting buffer release before read completion and completion acknowledgement before release.
- `src/emit.rs`: Converts GC types, let, for, closures, enum/match, and try/catch to ordinary WAT.
- `src/lib.rs`: `check(&str)` returns typed-core IR; `compile(&str)` supports both standard WAT and extensions. Also validates Wasm with wasmparser.

Enums become tags and case-specific GC subtypes. bytes/text uses private packed GC arrays, with UTF-8 validation for text.
User-defined functions, types, aliases, and closures are monomorphized through `(@type.params $T ...)` and `(@inst $name T...)`.
Ordinary generic `call` can omit type arguments uniquely determined by arguments and the expected type. Syntax changes made by macros preserve previously checked expressions; when a new instantiation changes the type environment, the whole module is checked again. In both cases, optimization follows the final type, effect, ownership, and race contract checks. See [macro boundaries and checking](user-macros.md#checking-scope-and-work-counters).
See the [map/fold example](../examples/generic-lists.walz), which takes concrete closure types, and the
[instantiation checking rules](generics.md).
Sugar for calling different closure types through a common interface, finally, and sugar for sharing a GC heap across threads remain outside this core.

The following type/effect design aligns with Verse's effect family and WASI 0.3.0 WIT.
It distinguishes Option/Result values, owned Future/Stream delivery handles, and suspends as a control effect.
See [SPEC.md](../SPEC.md) for design rules and implementation scope, and [async-contract.wit](../interfaces/async-contract.wit) for the shape contract.
Value types, effect contracts, ownership, laws, delivered/external Futures/Streams, WIT generation, Component ABI adapters, timer execution with JSPI, and task cancellation are implemented. Writers and language-side producer tasks remain future work.
`just wit-check` and WIT tests require `wasm-tools`; the verified version is 1.245.1.
