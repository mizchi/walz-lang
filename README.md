# Walz

English | [日本語](README.ja.md)

Walz is a WAT-superset language for building data-processing libraries on WebAssembly: search, aggregation, and binary processing engines that can be used from JavaScript and TypeScript.

It preserves WAT's S-expression syntax and Wasm semantics, adding generics and higher-level operations built on Wasm GC. Structured GC data and explicit SIMD instructions can be used in the same program, with type, effect, and ownership checks for the typed extension core.

Current examples include [Arquero-style table processing](docs/table.md), [column filtering and aggregation](docs/dictionary-column.md), [bitmap and numeric kernels](docs/simd-collections.md), [succinct indexes](docs/succinct.md), and [JSON processing](docs/json-values.md). Keep data inside a computation and batch work when useful; [dictionary queries](benchmarks/dictionary-column/README.md) and [table measurements](benchmarks/table/README.md) distinguish resident operations from construction, ABI conversion and complete input/output costs.

Its data abstractions are designed around `result`, `option`, `stream`, and `future` as used by [WASI 0.3](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0), to ease interoperation through WIT and the Component Model. These types are explicitly imported from `walz:core`. See the [runtime documentation](docs/runtime.md) for the current ABI and host support.

`walz:core` provides built-in data types and portable data operations. OS services and other I/O capabilities are supplied through explicit host imports.

Experimental [compiler parts written in Walz](docs/selfhost.md) include a parser, independent type/stack/effect validation and an integer/GC backend. A [bounded frontend](docs/selfhost-frontend.md) adds static modules and `@lib` libraries, fixed high-level forms, minimal core, monomorphization and hygienic expression macros. Full self-hosting remains future work; the Rust compiler is the main implementation. [Subset benchmark captures](docs/selfhost-benchmark.md) require canonical output parity with Rust and predate GC and the modular frontend.

- `@`-prefixed extensions for immutable bindings, loops, enums and pattern matching, named struct construction, and explicit closure captures.
- Monomorphized generics with constraints, explicit type arguments, and unambiguous call-site inference.
- Hygienic expression macros with explicit binding scopes and queries to the ordinary type checker.
- Type, effect, and ownership checking for the typed extension core.
- Output to Core Wasm, Core Wasm + JSPI, and WebAssembly components.
- Static `@lib` libraries and explicit Component composition with generic source constructors, inferred `@expose` / `@connect` adapters and `build --wit` interface generation. See [Component composition](docs/component-composition.md).
- CLI, browser playground, LSP, Vite ESM imports, and a TS7 Content Mapper.

```wat
(module
  (@use "walz:core" (type $result))
  (func $add (@type.params $T) (@where (Add $T))
    (param $a $T) (param $b $T) (result $T) (@effect pure)
    (+ $a $b))

  (func $main (export "main") (result i64) (@effect pure)
    (@let $answer (@inst $result i64 i32)
      (@result.ok (call $add 40 2))
      (@match $answer
        (@case ok ($value) $value)
        (@case err ($code) 0)))))
```

`@inst` supplies concrete type arguments; ordinary generic calls can infer them from arguments and the expected result. `@let` introduces an immutable binding, and `@match` handles both Result cases. The `main` function returns 42.

Walz S-expressions are the default source and printed form. `walz print file.walz` prints Walz; `walz print --format watz file.walz` selects an optional [Watz view](docs/review-syntax.md) for human review.

## Try it

Install the CLI with Cargo (Rust 1.99 or later).

```sh
cargo install --git https://github.com/mizchi/walz-lang --locked --bin walz
walz new app
cd app
walz run                    # 42
walz build                  # target/main.wasm
walz test
walz bench --list
walz bench --samples 10
```

From a checkout, install with `cargo install --path . --locked --bin walz`. The generator works offline and includes source, tests, a benchmark, bilingual references, and a portable [Walz skill](docs/agent-skills.md). Tests require Wasmtime with Wasm GC support; benchmarks require Node.js 24 or later. Laws, race-free contracts, and verified execution also use Lean. See the [CLI guide](docs/cli.md) for defaults and options, and [Getting started](docs/getting-started.md) for your first project.

Try the [hosted playground](https://mizchi.github.io/walz-lang/) for an interactive language tour, or run `just playground` locally (requires wasm-pack). The [browser playground](docs/playground.md) compiles Walz locally using the Rust compiler built to Wasm.

## Documentation

Start with the installation guide, then the language guide. The feature list records both implemented behavior and its limits.
Maintained guides are in English. README files, the cheat sheet, and specification also have Japanese editions. Frozen historical evaluations keep their original language.

| Read about | Documentation |
| --- | --- |
| Install, run, test, and benchmark a first project | [Getting started](docs/getting-started.md) · [CLI](docs/cli.md) |
| Learn the language through executable examples | [Language guide](docs/language-guide.md) |
| See features, supported boundaries, and limitations | [Feature list](docs/features.md) |
| Find a guide for a particular topic | [Documentation index](docs/README.md) |
| Look up syntax and contracts | [Cheat sheet](CHEATSHEET.md) / [日本語](CHEATSHEET.ja.md) · [Specification](SPEC.md) / [日本語](SPEC.ja.md) · [Core API](docs/stdlib-api.md) |
| Use Walz from JavaScript and TypeScript | [Vite ESM](docs/vite.md) · [TS7 Content Mapper](docs/content-mapper.md) |
| Read and write Walz with a coding agent | [Portable skill and CLI examples](docs/agent-skills.md) |
| Change the compiler or runtime | [Development](docs/development.md) · [Architecture](docs/architecture.md) |

See the [example catalog](examples/catalog.json), [initial sample assessment](ASSESSMENT.md), and [current limitations and TODO](TODO.md).
The move from the original Wasmtime prototype and the former name, gcwat, are documented in [MIGRATION.md](MIGRATION.md).

## License

[Apache License 2.0](LICENSE).
