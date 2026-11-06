# WLT

English | [日本語](README.ja.md)

WLT is a small extension of WAT built around Wasm GC and static analysis. It accepts standard WAT and marks language extensions with `%`.

- Immutable lexical bindings, enums and pattern matching, and explicit closure captures.
- Explicit imports of standard types and functions from `wlt:std`, with monomorphized generics.
- Type, effect, and ownership checking, with output to Core Wasm, Core Wasm + JSPI, and WebAssembly components.

```wat
(module
  (%import "wlt:std" (type $result))
  (func $main
    (export "main")
    (result i64)
    (%effect pure)
    (%let
      (($a i64 (i64.const 40))
       ($b (i64.const 2))
       ($sum
         (%inst $result i64 i32)
         (%result.ok (i64.add (local.get $a) (local.get $b))))
      )
      (%match (local.get $sum)
        (case ok ($value) (local.get $value))
        (case err ($code) (i64.const 0))))
  ))
```

Grouped `%let` bindings initialize from left to right. Later initializers can refer to earlier bindings. The `main` function above returns 42.

## Try it

You need Rust 1.97 or later, just, and a Wasmtime build with Wasm GC support.

```sh
git clone https://github.com/mizchi/wlt-lang.git
cd wlt-lang
just run examples/closure.wlt     # 42
just wat examples/closure.wlt     # inspect the generated WAT
just test
```

JavaScript execution requires Node.js 24 or later and pnpm. Verified execution also uses Lean. See [Getting started](docs/getting-started.md) for setup and commands.

## Documentation

The detailed documentation linked below is in Japanese.

| Topic | Documentation |
| --- | --- |
| Scalar instructions, Result sequencing, newtypes and early exits | [Core expressions](docs/core-expressions.md) |
| Syntax overview | [Cheat sheet](CHEATSHEET.md) · [Specification](SPEC.md) |
| Standard types, Future, and grouped bindings | [Standard modules](docs/standard.md) |
| Type parameters, bounds, and monomorphization | [Generics](docs/generics.md) |
| Built-in type classes, short-circuit logic and bit operations | [Type classes](docs/typeclasses.md) |
| v128 and vector operations | [SIMD](docs/simd.md) · [Kernels and benchmarks](docs/simd-kernels.md) |
| Named struct construction | [Struct construction](docs/struct-construction.md) |
| Imports, exports, and dead code elimination | [Static modules and DCE](docs/static-modules.md) |
| Effects, types, and memory regions | [Type checking and static analysis](docs/analysis.md) |
| Ownership and proofs | [Verified execution](docs/verification.md) · [External resources](docs/resources.md) |
| Async execution, ABI, and cancellation | [Runtime](docs/runtime.md) · [WIT bindings](docs/wit-bindings.md) |
| Tests and first-time reader evaluations | [Testing](docs/testing.md) · [Evaluation harness](evals/README.md) |
| Implementation structure | [Architecture](docs/architecture.md) |

See the [example catalog](examples/catalog.json), [current limitations](ASSESSMENT.md), and [TODO](TODO.md).
The move from the original Wasmtime prototype and the former name, gcwat, are documented in [MIGRATION.md](MIGRATION.md).
