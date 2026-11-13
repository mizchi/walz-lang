# Selfhost compiler benchmark v2

Recorded 2026-11-13T10:49:49.302Z on Apple M5, darwin/arm64, v24.22.2 (V8 13.6.233.17-node.63), rustc 1.99.0 (b940084d7 2026-09-28).

Core is faster than the full Rust compiler API on these eight inputs. Against the narrower Rust WAT parser plus validator, Core wins the small/control cases but is about 8% slower on the 32-function source and 21% slower on the 128-function source. These are different compiler pipelines; this does not establish a language-wide speed advantage.

## Resident compilation

Microseconds per source compilation; median of nine sample means. Lower is better. Rust/Core in the last column uses the narrower WAT reference; a value above 1 favors Core.

| Input | Source bytes | Rust full API | Rust WAT+validate | Core+value ABI | Component+value ABI | WAT/Core |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| integer-mix | 273 | 35.76 | 6.77 | 5.10 | 9.27 | 1.33× |
| i64-loop | 361 | 28.89 | 8.95 | 6.23 | 10.53 | 1.44× |
| recursive-call | 244 | 36.52 | 6.89 | 3.54 | 8.28 | 1.95× |
| branches-and-traps | 317 | 40.77 | 9.12 | 4.31 | 8.89 | 2.11× |
| i64-bits | 346 | 45.13 | 10.88 | 5.97 | 11.78 | 1.82× |
| wide-indices | 9630 | 1738.23 | 318.97 | 201.12 | 224.48 | 1.59× |
| expressions-32 | 9104 | 1359.45 | 132.65 | 143.06 | 157.72 | 0.93× |
| expressions-128 | 36081 | 4970.46 | 502.72 | 605.79 | 625.24 | 0.83× |

The full API takes about 4.6–10.3 times Core's time on this corpus. The WAT-only reference takes about 0.83–2.11 times Core's time. No outliers are removed. Generated-program runtime is not measured.

## Resident checking

Rust uses walz::validate and guests use compiler.check. The i64-loop input takes Rust's native-WAT fallback, which also encodes/validates a module; its check row is not a standalone typed-IR check.

| Input | Rust validate µs | Core µs | Component µs | Rust check route |
| --- | ---: | ---: | ---: | --- |
| integer-mix | 25.30 | 4.43 | 8.79 | structured |
| i64-loop | 55.18 | 5.51 | 11.89 | native WAT fallback |
| recursive-call | 26.28 | 3.05 | 6.93 | structured |
| branches-and-traps | 28.43 | 3.64 | 7.51 | structured |
| i64-bits | 31.58 | 5.11 | 9.62 | structured |
| wide-indices | 949.38 | 193.33 | 225.47 | structured |
| expressions-32 | 1090.51 | 130.51 | 143.59 | structured |
| expressions-128 | 4220.67 | 581.41 | 622.83 | structured |

## Output parity and sizes

All eight outputs have exactly equal canonical bytes for Rust, Core and Component. Raw Wasm bytes differ because the contract excludes debug-name sections, type interning, local grouping and LEB spelling. Canonicalization preserves signatures, function/local/export order and every instruction/immediate, including unreachable code. Unknown sections/types/opcodes fail closed. Equality therefore covers executable definitions for all arguments in this closed scalar subset; the execution probes are an additional check.

992 probes per implementation cover wrapping integers, i64 extremes, conversions, recursion, loops and traps. Each measured output must also equal that compiler's captured raw bytes. Snapshot file/hash mismatches abort the run.

| Input | Rust raw bytes | Core raw bytes | Component raw bytes | Canonical bytes equal |
| --- | ---: | ---: | ---: | --- |
| integer-mix | 86 | 64 | 64 | yes |
| i64-loop | 115 | 69 | 69 | yes |
| recursive-call | 79 | 56 | 56 | yes |
| branches-and-traps | 115 | 75 | 75 | yes |
| i64-bits | 133 | 86 | 86 | yes |
| wide-indices | 3222 | 2244 | 2244 | yes |
| expressions-32 | 1952 | 1782 | 1782 | yes |
| expressions-128 | 7616 | 6937 | 6937 | yes |

Compiler artifact sizes, distinct from generated-program sizes: Core GC module 51312 bytes; input Component binary 57337 bytes; native release benchmark peer 8291456 bytes. These are not equivalent deployable bundle sizes; runtime glue and transpiled modules are recorded separately in the capture manifest.

## Method and reproduction

At least 300 ms / 16 calls warmup per variant, calibrated calls targeting 10 ms of timed work per sample, nine samples, rotating variant order. Each call has two monotonic clock reads and retains that overhead. Builds, startup, loading/instantiation, JSON/file I/O, equality/execution checks and output destruction are outside clocks. Explicit Node GC and microtask draining occur between sampled batches; allocation-triggered GC remains included. No concurrent builds/tests/profiles were run during measurement.

Rust references are persistent release-mode peers. The full API runs additional source analyses; the WAT reference is pinned wat::parse_str plus wasmparser::Validator, and its raw bytes must equal the full API's native-WAT bytes. Core/Component include input copying, guest parsing/checking/encoding and output lifting. These measurements do not cover GC/imports, lexical let or general Walz macros.

Sources and probes are byte-identical to [v1](../20261113-v1/README.md). v2 adds the narrower WAT reference and reruns all timings; no v1 sample was replaced or relabelled. The suite/input hash fixture is [manifest.json](../../../benches/fixtures/selfhost-v2/manifest.json).

See [the benchmark guide](../../../docs/selfhost-benchmark.md). From the repository root:

```sh
just test-selfhost-bench
just verify-selfhost target/selfhost-bench/capture-v2
just bench-selfhost target/selfhost-bench/capture-v2 target/selfhost-bench/repeat-v2
# Or build a fresh current-source capture before measuring:
just capture-selfhost target/selfhost-bench/new-capture
```

The complete local frozen snapshot is /Users/mz/ghq/github.com/mizchi/wlt-lang/target/selfhost-bench/capture-v2. To replay the recorded harness rather than the current one, run its source/runtime/selfhost-bench.mjs with Node --expose-gc, that capture, and a new output directory. The compact public record contains [raw samples/methodology](results.json), [inputs/probes/output binaries](cases/), [source/tool hashes](capture.json), and code snapshots for provenance. Those compact code copies are not a standalone package; the full local snapshot includes the native executable and runtime adapters.
