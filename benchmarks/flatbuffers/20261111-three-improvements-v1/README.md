# Packed storage, synchronous calls, and ABI batching

Apple M5, Node 24.22.2, Rust 1.99. Sources, artifacts, inputs, raw samples and validation logs are retained. No SIMD or mandatory wasm-opt step was added.

## Implementation

1. The builder uses packed i8 wire storage and i32 field offsets. Growth and bytes/string writes use bulk copies; finish makes one immutable range snapshot without an intermediate i32 array. Wire-buffer element storage is four times smaller; total process memory is not four times smaller.
2. Idle synchronous API calls create no result/completion Promises or public Task handles, and reuse their private argument array. Cleanup state remains. Queued start(), JSPI, Future values, cancellation acknowledgments and disposal rules retain their contracts. Handling both Promise outcomes prevents an ignored rejection-handler descendant from re-assimilating a returned Future.
3. pipeline.walz adds inspect, transform, inspect-batch and transform-batch. Queries validate all known Message/Child fields and avoid materializing vectors at the output ABI. Transform performs decode → wrapping i32 addition → encode in Wasm; unknown fields are not preserved. Batches return one Result per item in order, continuing after invalid items. Inputs remain immutable. No persistent external handle pool was introduced.

Packed i8/i16 arrays, get_s/get_u, numeric array.copy, range snapshots and bytes-to-array copying are checked for storage types, effects and opaque representations. Operand evaluation, Wasm traps and overlap behavior are preserved. Existing unpacked i32 helper emission remains stable, so all historical frontend reference hashes still match.

## Paired comparison

Baseline: ../20261111-tuning-v1/current-v2. After: capture-final. Paired-v3 checks identical inputs/reference hashes, warms both sides, alternates order and uses equal iteration counts. Nine samples; automatic GC counts. Output equality, microtask draining and ABI cleanup checks are outside clocks. No profiler/build/test activity during timing. Values are medians in µs; speedup is the recorded median paired ratio.

| Case | Before µs | After µs | Speedup |
| --- | ---: | ---: | ---: |
| small.encode.resident | 0.22 | 0.12 | 1.82× |
| small.decode.resident | 0.11 | 0.11 | 1.01× |
| small.encode.core | 1.73 | 1.12 | 1.55× |
| small.decode.core | 1.77 | 1.14 | 1.53× |
| small.encode.component | 5.16 | 4.33 | 1.20× |
| small.decode.component | 5.01 | 4.16 | 1.19× |
| unicode-4k.encode.resident | 6.10 | 0.63 | 9.75× |
| unicode-4k.encode.core | 12.10 | 5.62 | 2.15× |
| unicode-4k.encode.component | 16.43 | 9.82 | 1.68× |
| bytes-64k.encode.resident | 121.42 | 12.67 | 9.61× |
| bytes-64k.encode.core | 178.68 | 64.04 | 2.78× |
| bytes-64k.encode.component | 196.63 | 80.22 | 2.43× |
| values-16k.encode.resident | 125.03 | 27.54 | 4.55× |
| values-16k.encode.core | 171.91 | 75.26 | 2.25× |
| values-16k.encode.component | 172.83 | 74.44 | 2.26× |

Large materialized decodes are essentially unchanged (about 0.98–1.05×). Packed storage improves encoding; per-byte ABI conversions remain.

## API shape comparison

Final suite: walz-flatbuffers-pipeline-v2, seven samples, eight messages per batch. Times below are per message including portable ABI transfers. All paths use the same newly optimized compiler/runtime, and every full output/empty arena/handle table is checked outside timing.

JS uses official ByteBuffer accessors, zero-copy Int32Array views on this aligned little-endian corpus, exact chunked Number-to-BigInt sums, a manual transform loop and fresh/reused official builders returning owned bytes. JS trusts valid input; Walz additionally validates layout and all known strings. JS wire patching would be a different task. This suite compares decode/map/re-encode.

| Workload / operation | Core materialized | Core query/fused | Core batch | Component materialized | Component query/fused | Component batch | JS best measured |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| small / inspect | 1.20 | 0.99 | 0.91 | 4.22 | 3.56 | 1.14 | 0.21 |
| small / transform | 2.50 | 0.92 | 0.79 | 8.80 | 3.81 | 1.40 | 0.80 |
| bytes-64k / inspect | 48.96 | 21.70 | 21.19 | 55.13 | 26.70 | 23.45 | 0.30 |
| bytes-64k / transform | 118.38 | 67.43 | 66.06 | 135.79 | 78.77 | 70.23 | 13.83 |
| values-16k / inspect | 67.88 | 37.41 | 36.97 | 77.05 | 42.12 | 38.15 | 14.86 |
| values-16k / transform | 133.48 | 107.27 | 107.79 | 149.99 | 114.79 | 108.29 | 59.15 |

Large queries/transforms improve against Walz’s materialized path but remain slower than optimized JS. Small Core batch transform is approximately tied with the reused JS builder, not a general JS speed advantage. Component benefits more from amortizing small-call overhead. Incoming bytes still copy into GC.

## Ordinary codec

Independent v2 run, nine samples, µs. Resident GC values exclude the portable ABI; Core includes it. JS reused builders return independent bytes.

| Input | JS fresh encode | JS reused encode | Resident encode | Core encode | JS decode | Resident decode | Core decode |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| small | 0.73 | 0.51 | 0.13 | 1.22 | 0.25 | 0.11 | 1.14 |
| unicode-4k | 6.04 | 1.92 | 0.67 | 5.84 | 1.46 | 0.61 | 5.72 |
| bytes-64k | 30.27 | 8.84 | 19.52 | 68.57 | 5.43 | 1.94 | 49.86 |
| values-16k | 70.42 | 49.56 | 33.31 | 68.92 | 5.83 | 22.22 | 58.84 |

## Size and profiling

Vite ES/oxc, gzip level 9, deployed assets compressed independently, inlined Wasm counted once. Maps/unused containers excluded. Binaryen 133 uses open-world O3/Oz and explicit features. Default builds do not invoke it.

| Variant | Gzip bytes |
| --- | ---: |
| Official JS | 4003 |
| core / original | 14742 |
| core / O3 | 11504 |
| core / Oz | 11504 |
| component / original | 33209 |
| component / O3 | 28049 |
| component / Oz | 28089 |

The basic codec bundle does not shrink: Core gzip increases from 14,668 to 14,742 B and Component from 33,122 to 33,209 B. Conversion helpers add a little code. The scalar Core JS graph still passes its existing 32 KiB budget. Optional pipeline exports are separate from main.walz.

CPU/heap profiles are separate from speed measurements (profiles/). bytes-64k.encode.core spends about 57% of sampled self time in byte ABI lifting/lowering and 15% in capacity allocation. values-16k.encode.core spends 40% in builder.i32s and 28% in byte lowering. ABI conversion and scalar i32 packing remain the main targets. Heap sampling does not cover all Wasm GC allocations; async_hooks directly checks zero Promise creation on the scheduler’s ordinary synchronous path.

## Validation and provenance

- Rust workspace: 663 passed, plus the new imported-macro regression in focused checks. Focused Rust tests, clippy, formatting, cheat-sheet/documentation/examples/skill checks pass. Native FlatBuffers: four passes. No Lean model was weakened; bulk operations remain outside the existing pure law fragment.
- Node: 187 cases; 186 passed in the combined run. The sole failure was a compiler subprocess SIGKILL with no diagnostic; both tests in that file passed in an isolated retry. Historical reference hashes pass. A stronger sum test above 2^53 also passes.
- Chromium: all 91 tests pass, including direct/bundled Core/Component FlatBuffers, packed copies, batches, JSPI and cancellation. Normal/error/cancelled cleanup and retention are exercised. Tests are not a proof of absence of all GC leaks or malformed-input bugs.
- capture/ is an unmeasured preliminary snapshot with sparse legacy helper emission, reverted to keep historical WAT stable.
- capture-final/ is the retained compiler/runtime. compare/, run/, sizes/ and profiles/ use it. pipeline/ retains the first pipeline-v1 run (TypedArray.map and bytewise JS query).
- capture-pipeline-v2/ improves only the pipeline measurement driver. Its 41 deployed artifacts are byte-identical to capture-final/ (artifact-equivalence.json). pipeline-v2/ is the final API comparison.
- validation/ retains successful checks, the combined SIGKILL failure and isolated retry. Existing frozen measurements/reference files were not overwritten.

```sh
just test-packed-arrays
just test-flatbuffers
node --test runtime/tasks-sync.test.mjs
just compare-flatbuffers benchmarks/flatbuffers/20261111-tuning-v1/current-v2 benchmarks/flatbuffers/20261111-three-improvements-v1/capture-final target/fb-compare-fresh
just bench-flatbuffers-pipeline benchmarks/flatbuffers/20261111-three-improvements-v1/capture-pipeline-v2 target/fb-pipeline-fresh
just size-flatbuffers benchmarks/flatbuffers/20261111-three-improvements-v1/capture-final target/fb-size-fresh target/wasm-opt-research/binaryen-version_133/bin/wasm-opt
```

Every output destination must be new.
