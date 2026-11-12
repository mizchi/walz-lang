# Synchronous direct GC and JS string ABI experiment

Apple M5, Node 24.22.2 (V8 13.6), Rust 1.99. Two opt-in experimental backends compile the same checked Walz source. The stable Core＋JSPI and Component targets are unchanged. See [implementation and tasks](../../../docs/js-native-experiment.md).

## Result

The largest size win comes from a smaller synchronous shared-value ABI, not JS string builtins alone. Ordinary direct UTF-8 output is 8,285 B gzip (43.8% below Core); with Binaryen 133 O3 it is 5,969 B, versus the official JS adapter's 4,003 B. Native JS strings add 143 B in this FlatBuffers example. They help string pass-through substantially, but do not remove FlatBuffers' UTF-8 wire encoding or the bytes/GC transfer loops.

This is an explicit capability tradeoff: direct profiles have no queued start(), cancellation, host resources/imports, Future/Stream or source memories. The compiler rejects modules needing those facilities after the full type/effect/ownership/law checks. Public shared JS values, strict UTF-8 validation, byte offsets, BOM retention, snapshots and malformed-input Results remain supported. The profile is experimental, not a new default CLI build target.

## Deployed size

Actual Vite ES/oxc bundles, gzip level 9; JS and deployed Wasm compressed independently, maps/unused files excluded. O3 is open-world Binaryen 133 with explicit features and trap semantics retained. Every optimized/bundled variant was instantiated and all codec fields checked.

| Variant | Gzip bytes | Official JS ratio |
| --- | ---: | ---: |
| Official JS + handwritten Message adapter | 4,003 | 1.00× |
| Stable Core | 14,742 | 3.68× |
| Stable Core + O3 | 11,504 | 2.87× |
| Direct UTF-8 | 8,285 | 2.07× |
| Direct UTF-8 + O3 | 5,969 | 1.49× |
| Direct JS strings | 8,428 | 2.11× |
| Direct JS strings + O3 | 6,086 | 1.52× |

Direct UTF-8 is 3,251 B JS + 5,034 B Wasm; its O3 version is 3,254 B JS + 2,715 B Wasm. Direct JS strings is 3,254 B JS + 5,174 B Wasm. Strict buffer validation remains in the Wasm. The official JS reader trusts valid input and does not provide equivalent structural verification. Its measured adapter is handwritten using official Builder/ByteBuffer, not a flatc-generated verifier.

The stable Core/Component's 16 deployed JS/Wasm files are byte-identical to ../20261111-three-improvements-v1/capture-final. The earlier bundle sizes therefore remain a valid baseline. The experiment does not attribute the entire difference to validation or to string builtins.

## Codec speed

56 variants, nine samples each; medians in µs. Setup is excluded, while actual ABI transfers, source computation and output copies count. Each variant warms at least 4,096 calls and 250 ms, calibrates near 20 ms, rotates order by round, and runs explicit GC before samples. Automatic GC inside clocks counts. Complete results, event-loop drain and arena/handle/scratch checks are outside clocks. No profiler/build/test activity ran during timing. Raw samples are in run-final/native.json.

| Input / operation | JS reused encode / materialized decode | Core | Component | Direct UTF-8 | Direct JS strings |
| --- | ---: | ---: | ---: | ---: | ---: |
| small / encode | 0.436 | 1.184 | 4.387 | 0.679 | 0.692 |
| small / decode | 0.245 | 1.156 | 4.262 | 0.962 | 0.962 |
| unicode-4k / encode | 1.583 | 5.785 | 9.523 | 4.602 | 4.634 |
| unicode-4k / decode | 1.319 | 5.444 | 8.930 | 5.074 | 5.090 |
| bytes-64k / encode | 8.058 | 65.524 | 71.798 | 61.045 | 61.598 |
| bytes-64k / decode | 5.228 | 48.164 | 53.232 | 44.193 | 44.142 |
| values-16k / encode | 45.935 | 67.375 | 73.780 | 61.917 | 58.756 |
| values-16k / decode | 5.475 | 55.658 | 62.824 | 49.230 | 49.517 |

Small encode benefits from lower fixed overhead. Large bytes/numeric vectors remain slower than the optimized official JS implementation because bytes still copy between JS transfer memory and GC arrays. This is not evidence that the whole codec replaces JS faster across workloads. O3 variants were size/functional checks; speed figures above are unoptimized Wasm.

## String-only operations

The larger input is `"A😀".repeat(13107)`, 65,535 UTF-8 bytes. It is constructed once outside clocks. Echo must still normalize invalid JS surrogates consistently with TextEncoder; native mode uses String.toWellFormed. Length means UTF-8 byte length in all backends, not JS UTF-16 length.

| Input / operation | Core | Direct UTF-8 | Direct JS strings |
| --- | ---: | ---: | ---: |
| small / echo | 0.811 | 0.381 | 0.066 |
| small / length | 0.481 | 0.242 | 0.250 |
| 64k / echo | 189.902 | 125.771 | 9.314 |
| 64k / length | 136.115 | 79.441 | 89.169 |

Native strings avoid conversion for echo. Byte length still needs UTF-8 encoding and does not get the same gain. This prototype uses the js-string cast builtin and imported string constants; existing std string algorithms still operate through UTF-8. It does not yet lower std concat/substring algorithms to the corresponding JS string builtins, and it does not require the separate JS Text Encoding Builtins proposal.

## Runtime and validation

- Full Rust workspace: 667 passed, 3 ignored. The new Rust tests verify module validation, portable shape checks, synchronous eligibility, used source-memory rejection and scalar-only modules. Clippy and formatting pass.
- Native UTF-8 and JS-string runtime tests cover Unicode/BOM/unpaired surrogates, byte length, typed numeric lists, nested lists/options, unit payloads, snapshots, integer limits, signed zero, bad JS arguments, reentry, malformed FlatBuffers and stable transfer-memory high-water capacity. Normal and error calls leave no host handle/arena tables.
- An additional WeakRef/GC test retains an error while dropping the runtime. It covers both guest traps and argument validation failures. It first exposed lazy V8 error stacks retaining a guest after argument failure; materializing the stack before publication fixes it in both modes. Intrinsic-host failures cannot be swallowed by source catch_all.
- Targeted Node regression run: 16 passed, including stable Core/Component codec and scheduler tests. Both new modes pass the stronger GC retention test.
- Chromium: six passed, covering direct and actual Vite-bundled native variants plus stable Core/Component codec, real JSPI and cancellation.
- Existing frontend reference hashes still match. No source type/effect/ownership gate or Lean contract was weakened. New ABI correctness is covered by validation/execution tests, not an end-to-end ABI proof.

## Evidence and replay

- capture/, run/ and sizes/ retain the initial trial, before the argument-error retention fix. They are not the final artifacts.
- capture-final/, run-final/ and sizes-final/ contain the retained runtime and final measurements. Captured inputs/reference code are unchanged between trials. Original earlier benchmarks were not overwritten.
- validation/ records the failing retention test, its successful fix, Rust/Node/browser checks and the current test sources. Transfer memory intentionally retains a bounded high-water capacity (64 KiB initially, at most 16 MiB); this is not an arena/host-resource leak.

```sh
just build-js-native examples/flatbuffers/main.walz target/fb-direct utf8
just build-js-native examples/flatbuffers/main.walz target/fb-js-strings js-strings
just test-js-native
just test-js-native-e2e
just bench-js-native benchmarks/flatbuffers/20261111-js-native-v1/capture-final target/fb-native-run-fresh
just size-js-native benchmarks/flatbuffers/20261111-js-native-v1/capture-final target/fb-native-size-fresh target/wasm-opt-research/binaryen-version_133/bin/wasm-opt
```

Every measurement output must be a fresh directory. Performance and size conclusions apply to this schema, corpus, runtime and restricted JS profile.
