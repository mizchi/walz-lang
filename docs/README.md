# Walz documentation

English | [日本語](README.ja.md)

Start with [getting started](getting-started.md), the [language guide](language-guide.md), and the [feature list](features.md).
Use the [cheat sheet](../CHEATSHEET.md) for syntax lookup and [specification](../SPEC.md) for precise contracts.
Each detailed guide records its supported behavior and limits.

Maintained guides are written in English. [README](../README.md), [cheat sheet](../CHEATSHEET.md), and [specification](../SPEC.md) have `.ja.md` counterparts.
Historical evaluations and dated measurements retain their original language and inputs. See [the documentation policy](development.md#documentation-language).

## Learn the language

| Topic | Guide |
| --- | --- |
| Install, scaffold, run, test, and benchmark | [Getting started](getting-started.md), [CLI](cli.md) |
| Functions, bindings, data, loops, generics, and owners | [Language guide](language-guide.md) |
| Implemented features and restrictions | [Feature list](features.md) |
| WAT boundaries, numbers, early exits, and Result sequencing | [Syntax](syntax.md), [expressions](core-expressions.md), [validator](wat-validator.md) |
| Core types, API signatures, and naming | [walz:core](standard.md), [generated API](stdlib-api.md), [naming](naming.md) |
| Type arguments, monomorphization, and built-in operators | [Generics](generics.md), [type classes](typeclasses.md) |
| Named construction, traversal, and field operations | [Structs](struct-construction.md), [high-level forms](high-level-forms.md) |
| Option, Result, strings, lists, dictionaries, and views | [Value operations](option-result.md), [lists](lists.md), [collections](collections.md), [views](views.md) |
| JSON, SIMD, and indexes | [JSON](json-values.md), [SIMD](simd.md), [succinct data](succinct.md) |
| File visibility and removal of unused definitions | [Static modules and DCE](static-modules.md) |
| Define syntax extensions | [Hygienic expression macros](user-macros.md) |
| Read and write Walz with an agent | [Portable Walz skill](agent-skills.md) |

## Execute and integrate

| Topic | Guide |
| --- | --- |
| Try Walz in a browser | [Playground](playground.md) |
| Import Walz as ESM in Vite | [Vite](vite.md) |
| Type-check without generated declarations | [TS7 Content Mapper](content-mapper.md) |
| JS ABI, Future/Stream, and cancellation | [Runtime](runtime.md), [batch transfers](stream-batches.md) |
| Writers, scoped child tasks, and deadlines | [Child tasks](scoped-tasks.md) |
| WIT, Components, and typed Wasm dependencies | [Bindings](wit-bindings.md), [native worlds](native-worlds.md), [Wasm @use](wasm-use.md) |
| Explicit standard I/O and files | [CLI](cli.md), [WASI value adapter](wasi-files.md) |
| Shared memory, atomics, and Workers | [Threads](threads.md), [Worker host](worker-host.md) |
| The checked shared-memory subset | [Atomic cells](atomic-cells.md), [readonly regions](readonly-regions.md) |

## Check, measure, and develop

| Topic | Guide |
| --- | --- |
| Types, effects, and memory analysis | [Static analysis](analysis.md), [JSON diagnostics and type queries](tooling.md) |
| Ownership, external resources, laws, and proof boundaries | [Verification](verification.md), [resources](resources.md), [independent kernels](../proof/README.md) |
| Editor integration and errors | [LSP](lsp.md), [diagnostics](diagnostics.md) |
| Language tests and benchmarks | [Testing](testing.md), [benchmarking](benchmarking.md) |
| Generated code and compiler performance | [wasm-opt](wasm-opt.md), [SIMD measurements](simd-kernels.md), [Criterion](frontend-performance.md) |
| Change the implementation and reproduce checks | [Development](development.md), [architecture](architecture.md) |

## Examples and historical records

Find small programs in the [example catalog](../examples/catalog.json).
Follow [CLI](../examples/cli/README.md) or [Vite](../examples/vite/README.md) for complete workflows;
see [log statistics](../examples/apps/log-stats/README.md), [the stock ledger](../examples/apps/stock-ledger/README.md),
and [the workflow planner](../examples/apps/workflow/README.md) for larger applications.

[TODO](../TODO.md) records remaining work; [MIGRATION](../MIGRATION.md) records the move from gcwat and the Wasmtime checkout.
[Evaluations](../evals/README.md), grammar-eval, readability, and dated measurements preserve historical inputs and results.
Use this index, the specification, and generated API for current syntax and behavior.
