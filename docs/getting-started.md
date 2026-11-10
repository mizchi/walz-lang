# Getting started with Walz

Walz extends WAT's S-expressions with Wasm GC data and checked high-level forms.
Start by running a small program, then add tests, benchmarks, and external interfaces.
See the [feature list](features.md) for supported behavior and the [language guide](language-guide.md) for a syntax walkthrough.

## Install and choose a runtime

Install with Cargo from Rust 1.99 or later. Walz is not published on crates.io.

```sh
cargo install --git https://github.com/mizchi/walz-lang --locked --bin walz
walz --version
```

From a checkout, run `cargo install --path . --locked --bin walz` at its root.
Put Cargo's installation directory on PATH. Using the installed CLI does not require just or pnpm.

| Task | Additional requirement |
| --- | --- |
| Type checking and Core Wasm builds | None; the compiler embeds the core library |
| Default `walz run` and `walz test` execution | Wasmtime with Wasm GC support |
| Core＋JSPI execution and benchmark measurements | Node.js 24+; external asynchronous work requires JSPI |
| Laws, `@race-free`, and verified execution | `lean +v4.34.1` through elan |
| Browser execution | Wasm GC; asynchronous execution also needs JSPI |

Try the [hosted Playground](https://mizchi.github.io/walz-lang/) without installing a compiler.
See [CLI](cli.md) and [runtime](runtime.md) for execution requirements.

## Create an application

```sh
walz new app
cd app
walz run                    # 42
walz build                  # target/main.wasm
walz test
walz bench --list
walz bench --samples 10
```

The generator works offline and refuses an existing destination, including an empty directory.
It creates `src/main.walz`, `src/math.walz`, `tests/math.test.walz`, and `app_bench.walz`, with English/Japanese README and cheat sheets.
The local `.agents/skills/walz` skill includes syntax and CLI references for an agent working on the application.
No Git repository is initialized and no dependencies are installed. See [agent skills](agent-skills.md).

## Write a project by hand

Use a separate empty directory for this walkthrough. Create `math.walz` for the calculation and `main.walz` for the entry point.

```wat
;; file: math.walz
(module
  (@export (func $add))
  (func $add (param $a i64) (param $b i64) (result i64)
    (@effect pure)
    (+ $a $b)))
```

```wat
;; file: main.walz
(module
  (@use "./math.walz" (func $add))
  (func $main (export "main") (result i64) (@effect pure)
    (call $add 40 2)))
```

```sh
walz check main.walz
walz run                  # 42
walz build                # target/main.wasm
walz wat main.walz         # 生成したWATを表示
```

`func`, `param`, `result`, and `call` are WAT syntax. `$a` abbreviates a local read, and `+` resolves to an appropriate Wasm instruction.
The parameter types determine that `40` and `2` are i64. There is no default integer width or implicit numeric conversion.

`(@effect pure)` bounds this function's semantic effects to the empty set.
`(@export (func $add))` exposes a name to other Walz files; `(export "main")` exposes a Wasm function.
See [static modules](static-modules.md) for these separate boundaries.

## Add a test

Create `tests/main.test.walz`. Tests select functions from the library's source API.

```wat
;; file: tests/main.test.walz
(module
  (@use "../math.walz" (func $add))
  (@test "add returns 42"
    (= (call $add 40 2) 42)))
```

```sh
walz test                 # tests/以下の*.test.walzを実行
walz test --list          # 検査と一覧表示。実行はしない
```

A `@test` condition is i32: zero fails and any other value succeeds. See [testing](testing.md).

## Add a benchmark

Create `answer_bench.walz`. The `*_bench.walz` suffix lets `walz bench` discover it.

```wat
;; file: answer_bench.walz
(module
  (@use "./math.walz" (func $add))
  (@bench "add"
    (param $i i32) (result i64)
    (call $add (i64.extend_i32_u $i) 2)))
```

```sh
walz bench --list
walz bench --samples 10
```

Vary the input with the iteration number and return the result. This tiny example demonstrates the runner; it is not a useful performance comparison.
See [benchmarking](benchmarking.md) for timing boundaries and options.

## Choose an execution boundary

| Use case | Entry point | Details |
| --- | --- | --- |
| Run numeric code in Core Wasm | `walz build`, `walz run` | [CLI](cli.md) |
| Return strings/Results to JS or await Promises | `walz run --target core-js`, `walz build --target core-js` | [Runtime](runtime.md) |
| Import functions into Vite | `import {add} from './math.walz'` | [Vite ESM](vite.md) |
| Check Walz imports with TS7 | A Content Mapper-capable host and `contentMappers` | [TS7 Content Mapper](content-mapper.md) |
| Expose a WIT interface | `walz build --target component` | [WIT bindings](wit-bindings.md), [native worlds](native-worlds.md) |
| Use files or standard I/O | Supply explicit host capabilities | [CLI](cli.md), [WASI value adapter](wasi-files.md) |

Core Wasm GC references are not automatically converted to JS strings or objects. Use a Core＋JSPI or Component adapter for portable JS values.
`walz:core` provides portable types and data operations; files and networking are supplied by a host.

Continue with the [language guide](language-guide.md). For a multi-file application, see [the CLI example](../examples/cli/README.md);
for larger workloads, see [log statistics](../examples/apps/log-stats/README.md) and [the workflow planner](../examples/apps/workflow/README.md).
Compiler setup and just tasks are covered separately in [development](development.md).
