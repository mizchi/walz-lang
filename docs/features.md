<a id="walzの機能一覧"></a>

# Walz Feature Reference

Walz preserves WAT S-expressions and adds typed high-level syntax built on Wasm GC.
Its main goals are small syntax, explicit boundaries, and straightforward static analysis.
This page indexes current implementations; linked documents define the detailed rules for each feature.
See [Getting started](getting-started.md) for setup and the [Language guide](language-guide.md) for writing programs.

<a id="構文型データ"></a>

## Syntax, Types, and Data

| Feature | Current support | Rules and restrictions | Details |
| --- | --- | --- | --- |
| Standard WAT | Validate and compile standard instructions/declarations with the Wasm validator | Does not add Walz ownership/effect guarantees to arbitrary WAT | [Syntax](syntax.md), [validator](wat-validator.md) |
| Typed functions | Parameter/result types, numeric instructions, call, if, return, labeled branches | Typed-core results are unit or one value, distinct from standard WAT multiple results | [Basics](language-guide.md), [expressions](core-expressions.md) |
| Locals and shorthand | Immutable `@let`, multiple bindings, `$x`, expected-type numbers/strings | No default numeric width or implicit numeric conversion | [Bindings](standard.md), [value operations](option-result.md) |
| Control structures | Stateful `@loop`/`@recur`, `@for`, `@for-each`, `@try` for Wasm exceptions | Recur is tail-only in the innermost loop; for-each uses shared lists and unit bodies | [Cheatsheet](../CHEATSHEET.md), [traversal](high-level-forms.md) |
| GC structs/arrays | Nominal references, recursive types, explicit `mut`, nullable references | GC is per agent; shared references do not imply immutable referents | [Cheatsheet](../CHEATSHEET.md) |
| Named construction/decomposition | `@struct.new`, `@$Type::{...}`, field patterns, field updates | Every field is required; no untyped object/map literals or spread | [Construction](struct-construction.md), [updates](high-level-forms.md) |
| Optional source view | `walz print` defaults to Walz; `--format watz` selects a reversible review view with `func`/`@export` and direct `match`/`if` expressions | Structural calls preserve all other forms; no JavaScript runtime or native TypeScript/LSP integration | [Review syntax](review-syntax.md) |
| Enum/match | Payload variants, recursive enums, `@case`/`@default`, exhaustiveness/duplicate checks | Distinct from automatic type narrowing or arbitrary nested patterns | [Language guide](language-guide.md), [cheatsheet](../CHEATSHEET.md) |
| Value abstractions | Option, result, tuple, list, string, bytes from `walz:core` | Types are explicitly imported; list elements and similar values have shared/nonunit constraints | [Core types](standard.md), [lists](lists.md) |
| Generics | Monomorphized functions/types/aliases/closures; type-argument inference for ordinary calls | Specify `@inst` when ambiguous; no polymorphic recursion or proof of entire unapplied templates | [Generics](generics.md) |
| Type constraints/operators | Shared/nonunit/callable and similar constraints; built-in Add/Mul/Eq/Ord and other classes | No user-defined class/impl or dynamic dictionary passing; use Wasm instructions for float comparisons | [Constraints](generics.md), [operators](typeclasses.md) |
| Type aliases/newtypes | Generic aliases and nominal newtypes distinguishing scalars | Explicit newtype wrap/unwrap; no automatic inheritance of underlying-type classes | [Expressions and newtypes](core-expressions.md) |
| Closures | Declare type/capture names/capture types and pass values at construction; generic closures supported | No implicit captures; closures with owned captures can be called once only | [Language guide](language-guide.md), [example](../examples/closure.walz) |
| Macros | Hygienic expression macros, repetition, binding scopes, inferred-type queries | Do not alter the reader or standard WAT; generated expressions pass ordinary checks | [Macros](user-macros.md) |

<a id="コアライブラリと計算処理"></a>

## Core Library and Computation

Enumerate the types/functions to use from `walz:core`. This module does not implicitly import OS services.

| Feature | Current support | Rules and restrictions | Details |
| --- | --- | --- | --- |
| String/bytes | UTF-8 strings, immutable byte sequences, search/split/integer parsing, builders | Byte positions differ from character positions; general Unicode normalization is not provided | [Generated API reference](stdlib-api.md), [core library](../walz/core/README.md) |
| List/dict/builders | Immutable lists, map/fold, mutable string-keyed dictionaries, list builders | Elements must be shareable; dictionaries are mutable GC references, with updates visible through aliases | [Lists](lists.md), [collections](collections.md) |
| Zero-copy views | Bytes-view/string-view/array-view reference ranges; validate bytes-view as string-view without copying | Views allocate small GC values too; a small view can retain an entire source buffer | [Views](views.md) |
| Fixed-width byte access | Explicit le/be integer/float reads from bytes/views and writes to packed i8 arrays | Unsigned byte offsets, complete-range checks, unaligned access; bytes remain immutable | [Byte access](byte-access.md) |
| Checked size arithmetic | i32/i64 unsigned checked add, multiply, and power-of-two alignment | Option failure; ordinary Wasm arithmetic still wraps; application limits remain explicit | [Checked integers](checked-integers.md) |
| JSON | Value construction/traversal/parsing/stringification implemented in Walz | Preserves number lexemes; consult APIs for conversion ranges between JSON numbers and binary floats | [JSON](json-values.md), [parser measurements](json-parser-bench.md) |
| SIMD | V128, Wasm SIMD instructions, kernels, bulk numeric-list ABI transfer | No ABI directly passing v128 to JS; use numeric lists or linear memory | [SIMD](simd.md), [kernels](simd-kernels.md) |
| SIMD collections | Immutable dense bitmap and i32 columns; Boolean selection, fused counts and exact sums | Agent-local GC storage; construction copies data; compare resident and ABI-inclusive costs separately | [Collections](simd-collections.md), [library](../walz/simd/README.md) |
| Dictionary columns | Unique string dictionaries with validated SIMD i32 codes; decoding, equality/IN selections and batched counts | Dictionary-local IDs, exact UTF-8 equality; no nullable or narrow-code representation yet | [Dictionary columns](dictionary-column.md) |
| Table processing | Immutable relational verbs, fused f64x2 filters, aligned scans, bounded bulk numeric I/O, Arquero-style JS/TS facade and portable batches | Lazy caches add memory; 640-KiB raw transfer cap; scalar aggregates/string I/O; partial Arquero compatibility | [Tables](table.md) |
| Compiler bootstrap | Walz-written parser, independent integer/GC validator/codegen, static modules, fixed forms, GC effect bounds, minimal core, bounded monomorphization and hygienic macros | Experimental subset; ownership/proof/ABI lowering and stage 1/2 checks remain | [Compiler parts](selfhost.md), [frontend contracts](selfhost-frontend.md) |
| Succinct data structures | Bitsets, rank/select, indexes, persistent Wasm instances, readonly shared payloads | Hosts must obey layout/publication/lifetime contracts | [Succinct](succinct.md) |

See the source-generated [Standard API reference](stdlib-api.md) for exact parameters, results, and effects.

<a id="静的解析所有権証明"></a>

## Static Analysis, Ownership, and Proofs

| Feature | Current support | Guarantee boundary | Details |
| --- | --- | --- | --- |
| Types/effects | Type matching and reads/writes/allocates/suspends/host/throws upper-bound checks | Pure can still trap, physically allocate, or diverge | [Analysis](analysis.md) |
| Owned values/external resources | Explicit moves, consumption obligations, exception/return-path checking, SDK cleanup on cancellation | Not exclusive ownership of general GC references or Rust-equivalent lifetimes | [Ownership](verification.md), [resources](resources.md) |
| Opaque/module boundaries | Hidden representations, explicit public names/aliases, static linking and DCE | Even unused ordinary functions are checked before removal; no cycles, wildcards, or Walz dynamic imports | [Static modules](static-modules.md) |
| Read-only analysis | Reports call graphs, reference provenance/escape, i32 ranges, array lengths, and more | Retains unknown/opaque regions; analysis results alone do not remove checks | [Analysis](analysis.md) |
| Laws/independent contract checking | Lean checks integer laws, ownership certificates, and restricted race-free contracts | Not a semantic-preservation or termination proof for the entire compiler/VM/host | [Verification](verification.md), [proofs](../proof/README.md) |
| Optimization/advice | Private tuple/Option/Result scalarization, immutable-range guard elimination, bounded helper inlining, reachability elimination, wasm-opt output, opt-in performance warnings | Escaping/owned carriers retain storage; unproved ranges and GC bounds checks remain; speed depends on the workload | [Value optimization](value-optimization.md), [range optimization](range-optimization.md), [wasm-opt](wasm-opt.md) |

<a id="非同期並行並列"></a>

## Async, Concurrency, and Parallelism

| Feature | Current support | Rules and restrictions | Details |
| --- | --- | --- | --- |
| Future/Stream | Owned endpoints, ready values, external Promise/AsyncIterable integration, writers/batch transfers | Shared payloads; close is required even at Stream termination | [Runtime](runtime.md), [batches](stream-batches.md) |
| Child tasks | Scoped spawn/join, cancellation, deadlines, backpressure | Cooperative concurrency within one instance, not instructions creating OS threads | [Child tasks](scoped-tasks.md) |
| Wasm threads | Shared memory, atomic load/store/RMW/cmpxchg/wait/notify/fence | Worker creation/delivery belong to the host; GC references are not shared across agents | [Threads](threads.md) |
| Race-free subset | Fixed atomic cells, regions immutable after publication, checked-load contracts | Arbitrary raw-pointer access is outside scope; hosts must obey the same contract | [Atomic cells](atomic-cells.md), [readonly regions](readonly-regions.md) |
| Worker hosts | Node worker_threads/browser Worker pools, queues, cancellation, join | Explicit ESM drivers; Walz closures are not transferred directly | [Worker host](worker-host.md) |

<a id="出力と外部連携"></a>

## Outputs and External Integration

| Entry point | Purpose | Current boundary | Details |
| --- | --- | --- | --- |
| Core Wasm | Embed in VMs; use numeric values, GC, linear memory | Supply host imports separately; GC references are not automatically converted to JS objects | [CLI](cli.md) |
| Core+JSPI | Lightweight runtime with JS-value codecs, Promises, and task cancellation | Public-call serialization and ABI-value restrictions apply | [Runtime](runtime.md) |
| Component source | `@lib`, `@world`, inferred `@expose` / `@connect`, generic source constructors, read-only `@component.type` queries of explicit connections, generated WIT and memory identity inspection | Public values need portable WIT types; resource queries require explicit owners; constructor queries require preceding connections; opaque/resource representations cannot be copied as arguments; no runtime generics | [Component composition](component-composition.md) |
| Component/WIT | Typed WIT binding generation, output for supported worlds, native async/resources | SDK/native worlds support different capabilities; not a full WASI implementation | [Bindings](wit-bindings.md), [native worlds](native-worlds.md) |
| Binary Component `@use` | Select a constructor; instantiate explicitly and connect checked ports | No implicit state sharing or initialization DCE; imports require explicit arguments; Core Wasm needs a wrapper | [Wasm dependencies](wasm-use.md) |
| Explicit I/O | CLI stdio, permitted file names, host capabilities | Grants no implicit OS-resource/network authority | [CLI](cli.md), [WASI value adapters](wasi-files.md) |
| Vite ESM imports | `.walz` named exports, `?init`, development updates, production assets, SSR, generated type declarations | Vite 8.3 or later and compatible CLI; inherits GC ABI and shared-memory host-contract restrictions | [Vite](vite.md) |
| TS7 Content Mapper | Type checking without declaration files, export navigation, watch | Requires supported native TS7; unsaved dependency-Walz buffers are not supplied in bulk | [Content Mapper](content-mapper.md) |

Result/option/future/stream shapes are designed with WASI 0.3/WIT conversion in mind.
Having these types does not mean all WASI APIs can run unchanged.

<a id="開発ツールとサンプル"></a>

## Development Tools and Examples

| Entry point | Support | Details |
| --- | --- | --- |
| `walz new` | Offline application scaffolding with source modules, tests, benchmarks, bilingual references, and a local coding-agent skill | [CLI](cli.md#new), [agent skills](agent-skills.md) |
| `walz check`, `types`, `type-at`, `analyze` | Structured diagnostics, type information, analysis results | [CLI](cli.md), [tooling APIs](tooling.md) |
| `walz test`, `walz bench` | In-language tests and benchmarks iterating within Wasm | [Testing](testing.md), [benchmarks](benchmarking.md) |
| `walz lsp` | Diagnostics, hover, go-to-definition, completion, macro expansion, declaration recovery from incomplete buffers | [LSP](lsp.md) |
| Playground | Editing/checking/execution and feature tours using the Wasm compiler | [Playground](playground.md) |
| Implementation measurements/evaluations | Criterion, guest profiling, fixed reading/diagnosis/authoring evaluations | [Performance](frontend-performance.md), [evaluations](../evals/README.md) |

See the [Example catalog](../examples/catalog.json) for small examples, and
[CLI project](../examples/cli/README.md), [Log statistics](../examples/apps/log-stats/README.md),
[Stock ledger](../examples/apps/stock-ledger/README.md), and [Workflow](../examples/apps/workflow/README.md) for multifile examples.
Remaining work is in [TODO](../TODO.md). Historical evaluation/measurement files retain their original inputs/results and should not be used as a current API reference.
