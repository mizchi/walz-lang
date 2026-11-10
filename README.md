# WLT

English | [日本語](README.ja.md)

WLT is a programming language that preserves WAT's S-expression syntax and remains a superset of standard WAT. Built for Wasm GC, it adds generics and higher-level operations to make WebAssembly programs easier to write while preserving Wasm semantics.

Its data abstractions are designed around `result`, `option`, `stream`, and `future` as used by [WASI 0.3](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0), to ease interoperation through WIT and the Component Model. These types are explicitly imported from `wlt:core`. See the [runtime documentation](docs/runtime.md) for the current ABI and host support.

`wlt:core` provides built-in data types and portable data operations. OS services and other I/O capabilities are supplied through explicit host imports.

- `%`-prefixed extensions for immutable bindings, loops, enums and pattern matching, named struct construction, and explicit closure captures.
- Monomorphized generics with constraints, explicit type arguments, and unambiguous call-site inference.
- Hygienic expression macros with explicit binding scopes and queries to the ordinary type checker.
- Type, effect, and ownership checking for the typed extension core.
- Output to Core Wasm, Core Wasm + JSPI, and WebAssembly components.
- CLI, browser playground, LSP, Vite ESM imports, and a TS7 Content Mapper.

```wat
(module
  (%use "wlt:core" (type $result))
  (func $add (%type.params $T) (%where (Add $T))
    (param $a $T) (param $b $T) (result $T) (%effect pure)
    (+ $a $b))

  (func $main (export "main") (result i64) (%effect pure)
    (%let $answer (%inst $result i64 i32)
      (%result.ok (call $add 40 2))
      (%match $answer
        (case ok ($value) $value)
        (case err ($code) 0)))))
```

`%inst` supplies concrete type arguments; ordinary generic calls can infer them from arguments and the expected result. `%let` introduces an immutable binding, and `%match` handles both Result cases. The `main` function returns 42.

## Try it

Install the CLI with Cargo (Rust 1.99 or later).

```sh
git clone https://github.com/mizchi/wlt-lang.git
cd wlt-lang
cargo install --path . --locked --bin wlt
wlt run examples/closure.wlt              # 42
wlt build examples/closure.wlt -o target/closure.wasm
wlt test examples/cli/tests
wlt bench examples/cli/answer_bench.wlt
```

You can also install directly with `cargo install --git https://github.com/mizchi/wlt-lang --locked --bin wlt`. Tests require Wasmtime with Wasm GC support; benchmarks require Node.js 24 or later. Laws, race-free contracts, and verified execution also use Lean. See the [CLI guide](docs/cli.md) for defaults and options, and [Getting started](docs/getting-started.md) for your first project.

Try the [hosted playground](https://mizchi.github.io/wlt-lang/) for an interactive language tour, or run `just playground` locally (requires wasm-pack). The [browser playground](docs/playground.md) compiles WLT locally using the Rust compiler built to Wasm.

## Documentation

Start with the installation guide, then the language guide. The feature list records both implemented behavior and its limits.
Detailed guides are primarily in Japanese; the macro and LSP references are in English.

| Read about | Documentation |
| --- | --- |
| Install, run, test, and benchmark a first project | [Getting started](docs/getting-started.md) · [CLI](docs/cli.md) |
| Learn the language through executable examples | [Language guide](docs/language-guide.md) |
| See features, supported boundaries, and limitations | [Feature list](docs/features.md) |
| Find a guide for a particular topic | [Documentation index](docs/README.md) |
| Look up syntax and contracts | [Cheat sheet](CHEATSHEET.md) · [Specification](SPEC.md) · [Core API](docs/stdlib-api.md) |
| Use WLT from JavaScript and TypeScript | [Vite ESM](docs/vite.md) · [TS7 Content Mapper](docs/content-mapper.md) |
| Change the compiler or runtime | [Development](docs/development.md) · [Architecture](docs/architecture.md) |

See the [example catalog](examples/catalog.json), [initial sample assessment](ASSESSMENT.md), and [current limitations and TODO](TODO.md).
The move from the original Wasmtime prototype and the former name, gcwat, are documented in [MIGRATION.md](MIGRATION.md).
