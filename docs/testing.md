<a id="言語内のテスト"></a>

# In-language tests

For performance measurements, use `@bench` in `*_bench.walz` and `walz bench`. See [benchmarking](benchmarking.md).

Generate standard API documentation with `just stdlib-docs`. `just test-stdlib-docs` checks that public signatures, constraints, and effects match the documentation and that private type representations are absent.

Place a name and an `i32` condition directly under the module. 0 means failure; any other value means success.

```wat
(module
  (func $double (param $n i64) (result i64)
    (i64.mul (local.get $n) (i64.const 2)))

  (@test "double returns 42"
    (i64.eq (call $double (i64.const 21)) (i64.const 42))))
```

```sh
just test-language
just test-language tests/programs GC
just list-tests tests/programs
walz test tests/programs --filter closure
walz test tests/programs/core.test.walz --timeout-ms 1000
```

By default, `walz test` recursively discovers `*.test.walz` files under `tests/`. Explicitly specified files are accepted regardless of name.
Multiple paths are supported, and each file runs only once. Directory traversal does not follow symbolic links.
`target`, `node_modules`, and `.git` are excluded from recursive discovery unless explicitly specified as input paths. See also [installation and CLI](cli.md).
`--filter` matches test-name substrings; `--list` only type-checks and lists tests. The entire module is checked before filtering.
Other tests/files continue after failures, traps, and compilation errors. Zero tests is not treated as success.
Exit codes are 0 for success, 1 for test failures/runtime-environment/source errors, and 2 for invalid arguments.

Each test runs in a new Wasmtime process and instance. The default timeout is five seconds per test, including startup and compilation.
Change it with `--timeout-ms`. A timeout terminates the process and proceeds to the next test. Execution requires Wasmtime on PATH.
Runtime diagnostics for failed tests are displayed up to 64 KiB. As with ordinary run, the runner provides no import implementations.

Test bodies are converted to ordinary typed IR and run on Wasm GC. No implicit captures, shared mutable state, or dynamic function registry is added.
GC allocation, reference identity, explicit mutation, traps, and evaluation order match ordinary code. See [SPEC.md](../SPEC.md) for type/effect checks and supported domains.
The 21 tests across [core.test.walz](../tests/programs/core.test.walz), [gc.test.walz](../tests/programs/gc.test.walz), [macros.test.walz](../tests/programs/macros.test.walz), [values.test.walz](../tests/programs/values.test.walz), [ownership.test.walz](../tests/programs/ownership.test.walz), and [delivery.test.walz](../tests/programs/delivery.test.walz) also run through `just test`.
The Rust API, `walz::runner::run(paths, options)`, returns structured per-test results and per-file errors.

<a id="corejspi-のテスト計測"></a>

## Core＋JSPI tests and benchmarks

`walz test --target core-js --bindings policy.json --host host.mjs` and
`walz bench --target core-js --bindings policy.json --host host.mjs` can execute asynchronous imports and child tasks.
Each case gets its own process, Wasm instance, and host factory, and waits for host cleanup after success, failure, or cancellation.
`--list` only type-checks; it does not start the host module or Node.
A timeout requests cooperative cancellation. A host that has not exited after a cleanup grace period of up to five seconds is forcibly terminated and reported as a failure.

Core＋JSPI benchmarks measure end-to-end execution by awaiting each call in sequence. They include codecs, JSPI, and waiting time; results are limited to ABI-supported shared values or unit.
These conditions differ from the default `core` in-Wasm batch measurements. JSON records the distinction in `options.target` and `summary.runtime`.

<a id="ユーザー定義マクロの回帰と評価"></a>

## User-defined macro regressions and evaluations

```sh
just test-user-macros
just test-macro-tools
just test-macro-composition
just fuzz-user-macros 20261109 64
```

Checks cover syntax, hygiene, inference, LSP, repeated bindings/delegation, resuming checks from expressions, inference's temporary locals and labels, and rejection of deep expansions. Differential fuzzing compares macros, independent handwritten expansions, and a model of values/effects with optimization on/off in Wasmtime/V8. The first task includes Core/Component and Chromium checks and requires Node.js 24 or later and installed development dependencies.

Whether users can implement intended operations, diagnostics lead to the next fix, and increasing scale remains practical is assessed separately in [macro usability evaluations](../evals/README.md#マクロの実用性と規模の評価). Required correctness is separated from unmet exploratory tasks; fixed inputs, expectations, and rubrics are not overwritten.
