<a id="ベンチマーク"></a>

# Benchmarking

Write `@bench` in `*_bench.walz` and run it with `walz bench`. Requires Node.js 24 or later.
For Rust-side type-checking and compilation time, see [measuring the frontend with Criterion](frontend-performance.md).
For the Walz-written compiler, see [selfhost benchmarks and output parity](selfhost-benchmark.md).
For browser payloads, see [playground compiler deployment size](playground-size.md).
For library comparisons, see the [FlatBuffers harness](../walz/flatbuffers/README.md#compare-performance),
which separates resident Wasm GC calls from Core/Component value ABI costs and compares with official JavaScript.
`just bench-flatbuffers-pipeline capture output` measures checked queries, fused
transforms and batches against materialized APIs and optimized JavaScript. See the
[packed-storage and batching results](../benchmarks/flatbuffers/20261111-three-improvements-v1/README.md).

```wat
(module
  (@bench "integer mix"
    (param $i i32)
    (result i32)
    (^ (* $i 16777619) (>>> $i 7))))
```

```sh
just bench                          # カレント以下の *_bench.walz
just bench examples 'JSON'          # 名前の部分一致
just list-benches examples
walz bench examples/benchmarks/stdlib_v2_bench.walz --samples 20
walz bench examples --json > target/bench.json
walz bench examples --iterations 1000 --warmup-ms 100
just test-bench
```

The name and body are required. Names must be unique within a module, and names containing only whitespace are rejected.
One optional `(param $i i32)` and one optional `(result type)` are allowed, in that order.
Omitting the result type gives unit, as in `(@bench "unit" (nop))`.
The body is a single expression subject to normal type, effect, and ownership checks. Use ordinary `func` helpers
and explicit `@use` declarations for libraries. Place bench declarations in the executable entry file.
They cannot appear in files imported as libraries.

The `i32` iteration index starts at 0 in each batch. The body is called once for every iteration in the batch.
Omitting the parameter gives a body with no arguments. Fix `--iterations` when comparing under unchanged conditions.

Return values are stored in a public Wasm global so that computation results remain observable.
GC references and `v128` are also supported. Only the last result is retained; earlier results can be collected once unreachable.
Owned values and borrows cannot be returned. Consume or close resources inside the body and return a shared value or unit.
If a unit body uses `drop` on a computation's result, normal optimization may eliminate that computation.
Retaining the return value does not prevent constant folding or loop-invariant code motion, so vary the measured input using the iteration index or similar data.

<a id="計測と表示"></a>

## Measurement and output

Each case gets a separate Node process and Wasm instance. Calibration, warmup,
and every sample use that same instance. Walz/Wasm compilation, process startup,
instantiation, calibration, and warmup are excluded from sample times.

Iterations run in a Wasm loop; JS reads a monotonic clock only before and after each batch.
The result is batch time divided by the iteration count, in ns/op. It includes the iteration loop, result storage, the per-batch JS/Wasm call,
and GC and JIT effects. This overhead dominates very small expressions, so measure practical units of work.
Constructing fixed literals is also measured if it occurs in the body.

| Option | Default | Meaning |
| --- | --- | --- |
| `--samples N` | 10 | Number of measured batches, from 1 to 10000 |
| `--sample-ms N` | 10 | Target milliseconds per automatically calibrated batch; a positive integer |
| `--iterations N` | automatic | Iterations per batch, from 1 to 2147483647. Skips calibration when specified |
| `--warmup-ms N` | 100 | Warmup time before measurement; 0 skips warmup |
| `--timeout-ms N` | 30000 | Per-case timeout, including Node startup, Wasm compilation, and calibration |
| `--filter TEXT` | none | Substring match on bench names |
| `--list` | none | Type-check everything and list matching cases only; does not require Node |
| `--json` | none | Structured results in `walz-bench-v1` format |

Automatic calibration doubles the iteration count until it reaches the target time or the upper limit, then fixes that count.
Output includes the median, mean, minimum, maximum, sample standard deviation, sample count, and iteration count.
JSON also records raw samples in measurement order, Node/V8 versions, OS and architecture, options, and failure reasons.
Outliers are neither removed nor adjusted. Save the comparison environment, inputs, options, and results together.

The execution target is closed, synchronous guest code. The runner does not provide host import implementations or JSPI/Component asynchronous execution.
Exports other than the selected bench are removed from the link roots, so unrelated imports are not pulled in.
Discovery and execution are separate from ordinary `@test` cases; `walz test` does not run benches.

Directories are searched recursively without following symbolic links.
`target`, `node_modules`, and `.git` are excluded from recursive discovery, but can be targeted by explicitly specifying them as input paths.
Explicit files are accepted regardless of their names; duplicates are merged by normalized path. Everything after `--` is treated as a file path.
Other cases continue after traps, timeouts, or file type errors.
Zero cases is an error. Exit codes are 0 for success, 1 for measurement/source/environment errors, and 2 for invalid arguments.

The Rust API is `walz::benchmark::run(paths, options)`. To build a harness from checked IR,
use `walz::benchmark::compile(source, &module, index)`.
The [standard library benchmark examples](../examples/benchmarks/stdlib_v2_bench.walz) cover integers, views, and JSON parsing.

The batch iterations, monotonic clock, and result retention were informed by
[MoonBit's bench implementation](https://github.com/moonbitlang/core/blob/main/bench/bench.mbt). Walz represents these through
declarations, the CLI, and observable return values, without introducing a dedicated bench object.

<a id="プロファイルを使ったstdの改善"></a>

## Improving std with profiles

The [initial improvements and fixed measurement data](../benchmarks/stdlib-json/20261106-v1/report.md) record the JSON escaping change before and after.

Capture the pre-change `@bench` as Wasm, and collect profiles and speed comparisons in separate runs.
See [std_tune_v2_bench.walz](../examples/benchmarks/std_tune_v2_bench.walz). It covers 64/1024-byte concatenation,
JSON serialization with ASCII, Unicode, and escapes, builders, JSON parsing, and an input-construction-only control.
Literal construction is included in the body, so do not interpret the results as processing-only time.
v2 uses `$json.stringify` and `$json.from-string`. Inputs for the old API retain their original contents in `stdlib-v1.walz`/`std-tune-v1.walz` and are excluded from automatic discovery.
Rerun v1 measurements with the saved Wasm. Do not treat v2, whose API and inputs changed, as the same comparison suite as v1.

```sh
just bench-capture examples/benchmarks/std_tune_v2_bench.walz target/std-tune/before
just bench-profile target/std-tune/before target/std-tune/profile-before 'json.stringify string ASCII 1024' 200000

# stdを修正し、機能検査を通した後
just test-std-tune
just bench-capture examples/benchmarks/std_tune_v2_bench.walz target/std-tune/after
just bench-compare target/std-tune/before target/std-tune/after target/std-tune/comparison.json
just bench-profile target/std-tune/after target/std-tune/profile-after 'json.stringify string ASCII 1024' 200000
```

capture saves Wasm, inputs, expanded source, the measurement driver, and stdlib sources, and records SHA-256 hashes.
It does not overwrite directories or result files. Before profile/compare, the recorded metadata is checked against the actual files.
Different comparison inputs or drivers are rejected. Measurements use the fixed driver and Wasm.
If inputs change after measurement, save them as a separate suite/snapshot and take a new baseline.

compare passes both sides the same iteration count calibrated on the baseline. After warmup, it
alternates before→after and after→before runs. It saves raw samples, the median of paired sample ratios, Wasm sizes, and whether the binaries are identical.
Scalar results are checked for equality. This comparison cannot check the contents of GC references, so run separate functional checks using an oracle.
`just test-std-tune` checks exact JSON escaping, Unicode, concatenation/builders, and ABI cleanup on both Core and Component.

profile runs the same Wasm separately in V8 and Wasmtime, producing `.cpuprofile` files readable by Chrome DevTools and similar tools,
`.guest.json` files readable by Firefox Profiler, and per-function sample totals.
V8 samples for about one second after warmup; Wasmtime samples the specified iterations at 1 ms intervals.
Profile startup and Wasm compilation are outside the collection interval. Do not use profiled times for speed comparisons.
If Wasmtime yields too few samples, increase the iteration count with a new output destination.
Inlining and GC differ between the engines, so do not interpret cross-engine sample ratios as performance ratios.

<a id="corejspi-のテスト計測"></a>

## Core＋JSPI tests and benchmarks

`walz test --target core-js --bindings policy.json --host host.mjs` and
`walz bench --target core-js --bindings policy.json --host host.mjs` can execute asynchronous imports and child tasks.
Each case gets its own process, Wasm instance, and host factory, and waits for host cleanup after success, failure, or cancellation.
`--list` only type-checks; it does not start the host module or Node.
A timeout requests cooperative cancellation. A host that has not exited after a cleanup grace period of up to 5 seconds is forcibly terminated and reported as a failure.

Core＋JSPI benchmarks measure end-to-end execution by awaiting each call in sequence. They include codecs, JSPI, and waiting time; results are limited to ABI-supported shared values or unit.
These conditions differ from the default `core` in-Wasm batch measurements. JSON records the distinction in `options.target` and `summary.runtime`.
