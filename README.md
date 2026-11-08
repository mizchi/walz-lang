# WLT

English | [日本語](README.ja.md)

WLT is a programming language that preserves WAT's S-expression syntax and remains a superset of standard WAT. Built for Wasm GC, it adds generics and higher-level operations to make WebAssembly programs easier to write while preserving Wasm semantics.

Its data abstractions are designed around `result`, `option`, `stream`, and `future` as used by [WASI 0.3](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0), to ease interoperation through WIT and the Component Model. These types are explicitly imported from `wlt:core`. See the [runtime documentation](docs/runtime.md) for the current ABI and host support.

`wlt:core` provides built-in data types and portable data operations. OS services and other I/O capabilities are supplied through explicit host imports.

- `%`-prefixed extensions for immutable bindings, loops, enums and pattern matching, named struct construction, and explicit closure captures.
- Monomorphized generics with explicit type arguments and constraints.
- Type, effect, and ownership checking for the typed extension core.
- Output to Core Wasm, Core Wasm + JSPI, and WebAssembly components.

```wat
(module
  (%use "wlt:core" (type $result))
  (func $add (%type.params $T) (%where (Add $T))
    (param $a $T) (param $b $T) (result $T) (%effect pure)
    (+ $a $b))

  (func $main (export "main") (result i64) (%effect pure)
    (%let $answer (%inst $result i64 i32)
      (%result.ok (call (%inst $add i64) 40 2))
      (%match $answer
        (case ok ($value) $value)
        (case err ($code) 0)))))
```

`%inst` supplies concrete type arguments, `%let` introduces an immutable binding, and `%match` handles both Result cases. The `main` function returns 42.

## Try it

Install the CLI with Cargo (Rust 1.99 or later).

```sh
git clone https://github.com/mizchi/wlt-lang.git
cd wlt-lang
cargo install --path . --locked --bin wlt
wlt build examples/closure.wlt -o target/closure.wasm
wlt test tests/programs
wlt bench examples/benchmarks/stdlib_v2_bench.wlt
```

You can also install directly with `cargo install --git https://github.com/mizchi/wlt-lang --locked --bin wlt`. Tests require Wasmtime with Wasm GC support; benchmarks require Node.js 24 or later. Verified builds also use Lean. See the [CLI guide](docs/cli.md) for defaults and options, and [Getting started](docs/getting-started.md) for development setup.

For an interactive language tour, run `just playground` (requires wasm-pack). The [browser playground](docs/playground.md) compiles WLT locally using the Rust compiler built to Wasm.

## Documentation

The detailed documentation linked below is in Japanese.

| Topic | Documentation |
| --- | --- |
| Edit, compile and run in the browser | [Playground](docs/playground.md) |
| Scalar instructions, Result sequencing, newtypes and early exits | [Core expressions](docs/core-expressions.md) |
| Syntax overview | [Cheat sheet](CHEATSHEET.md) · [Specification](SPEC.md) |
| Shared memory, atomics, and host workers | [Wasm threads](docs/threads.md) · [Atomic-cell verification](docs/atomic-cells.md) |
| Async Worker pools and immutable parallel indexes | [Worker host](docs/worker-host.md) · [Readonly regions](docs/readonly-regions.md) · [Succinct indexes](docs/succinct.md) |
| Built-in types, Future, and grouped bindings | [Core module](docs/standard.md) |
| Type parameters, bounds, and monomorphization | [Generics](docs/generics.md) |
| Built-in type classes, short-circuit logic and bit operations | [Type classes](docs/typeclasses.md) |
| v128 and vector operations | [SIMD](docs/simd.md) · [Kernels and benchmarks](docs/simd-kernels.md) |
| Named struct construction | [Struct construction](docs/struct-construction.md) |
| List traversal, field patterns, and field updates | [High-level forms](docs/high-level-forms.md) |
| Option/Result helpers and expected string literals | [Value operations](docs/option-result.md) |
| Imports, exports, and dead code elimination | [Static modules and DCE](docs/static-modules.md) |
| Effects, types, and memory regions | [Type checking and static analysis](docs/analysis.md) |
| Ownership and proofs | [Verified execution](docs/verification.md) · [External resources](docs/resources.md) |
| Async execution, ABI, and cancellation | [Runtime](docs/runtime.md) · [WIT bindings](docs/wit-bindings.md) |
| CLI bundles, explicit file I/O, and child tasks | [CLI](docs/cli.md) · [WASI adapter](docs/wasi-files.md) · [Spawn/join](docs/scoped-tasks.md) |
| Typed macros with numeric WAT memory instructions | [Validator bridge](docs/wat-validator.md) |
| Tests, benchmarks and first-time reader evaluations | [Testing](docs/testing.md) · [Benchmarking](docs/benchmarking.md) · [Evaluation harness](evals/README.md) |
| Rust compiler benchmarks and profiling | [Frontend performance](docs/frontend-performance.md) |
| Editor and native components | [LSP](docs/lsp.md) · [Native WIT worlds](docs/native-worlds.md) |
| Implementation structure | [Architecture](docs/architecture.md) |

See the [example catalog](examples/catalog.json), [initial sample assessment](ASSESSMENT.md), and [current limitations and TODO](TODO.md).
The move from the original Wasmtime prototype and the former name, gcwat, are documented in [MIGRATION.md](MIGRATION.md).
