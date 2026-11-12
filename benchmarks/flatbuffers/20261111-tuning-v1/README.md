# FlatBuffers CPU, allocation, and bundle investigation

Apple M5 / macOS arm64 / Node v24.22.2 / V8 13.6.233.17-node.63.
The reference remains FlatBuffers JavaScript 25.9.23. Source-level improvements
use ordinary checked Walz; no compiler primitives or linear-memory scratch space
were introduced. The SIMD candidate was tested and not retained.

## Evidence

| Record | Contents |
| --- | --- |
| [Before profiles](profile-before/profile.json) / [after profiles](profile-after/profile.json) | 24 CPU profiles and separate allocation samples on each side |
| [Scalar comparison](compare-scalar-v3/comparison.json) | Final paired v3 timing of original vs retained source changes |
| [O3 comparison](compare-O3-v3/comparison.json) | Original vs retained sources plus optional Binaryen O3 |
| [SIMD comparison](compare-simd-v3/comparison.json) | Scalar vs experimental four-i32 SIMD writer |
| [Before sizes](size-before-v2/size.json) / [after sizes](size-after-v2/size.json) | Minified bundles, raw/gzip/Brotli sizes, module attribution and Wasm sections |
| [Sync-mode probe](sync-mode-probe.json) | Pinned jco `sync` vs `jspi` output is byte-identical for this input |
| [Original capture](../20261111-v1/capture/capture.json) | Original sources, inputs, Wasm and JS |
| [Scalar capture](after-scalar/capture.json) / [SIMD capture](after-simd/capture.json) | Retained and rejected implementations |
| [O3 capture](after-O3/capture.json) | Optional postprocessing, exact arguments, binary hash and transformed assets |
| [Current v2 benchmark](run-current-v2/report.md) / [raw results](run-current-v2/report.json) | Fresh JS/resident/Core/Component comparison using the corrected timer |
| [Current v2 capture](current-v2/capture.json) | Retained implementation with the updated measurement driver |

Open individual `.cpuprofile` files under `profile-before/` or `profile-after/`
in Chrome DevTools. `.heapprofile` files are sampled allocation profiles from a
separate pass. They report samples attributed by V8, not a complete census of
Wasm GC allocation or retained memory. Profiler timings are not speed benchmarks.

## CPU and allocation findings

Before tuning, numeric-vector decode spent about 91% of its resident CPU samples
in `$fb.i32s`. It called the generic scalar reader for each value, including a
width loop, i64 arithmetic, and repeated range/alignment checks. The retained
path validates the entire immutable vector first, then assembles four bytes with
i32 instructions per value. Wasm bounds checks remain, and malformed-input
Result errors are still checked through both backends.

Large encoding concentrated samples in capacity preparation/growth, final copying,
and byte writes. The retained builder selects the final power-of-two capacity
before allocating once, caches the write head/storage during vector copies, and
uses direct four-byte writes for i32 values. It preserves the 64 MiB limit,
default 64-byte initial buffer, little-endian layout and sealing checks.

After tuning, `finish` still takes about 48–51% of resident large-encode samples,
and `ensure` about 23–24%. It copies through an intermediate i32 array before
producing packed immutable bytes. This is the next source-level target.

On the JS Core path, 64 KiB decode spent about 35% in each of the input/output
buffer converters before tuning; the combined share remains around 72% afterward.
For small Core calls, GC was about 25–29% of samples. Allocation samples point to
binding calls, `Tasks.create`, public task handles and Promise construction.
Optimizing the parser alone does not remove these per-call costs.

CPU percentages are sampled self-time, with V8 inlining affecting attribution.
Do not interpret a changed percentage as an absolute duration change.

## Retained speed changes

Paired v3 measurement, nine batches, median µs/op. Identical inputs and reference
code; both sides run in one process, use the same per-case iteration count, and
alternate order. Warmup is at least 4,096 calls and 250 ms per side. Explicit GC
is outside the clock; automatic GC inside each batch counts. All values and ABI
cleanup are checked outside timing.

| Operation | Before | Retained source | Paired speedup |
| --- | ---: | ---: | ---: |
| 64 KiB bytes encode, resident | 170.91 | 121.34 | 1.41× |
| 64 KiB bytes encode, Core API | 213.58 | 168.17 | 1.27× |
| 16,384 i32 encode, resident | 177.70 | 120.12 | 1.48× |
| 16,384 i32 decode, resident | 38.87 | 21.37 | 1.81× |
| 16,384 i32 decode, Core API | 75.46 | 57.49 | 1.30× |

Small calls are approximately unchanged; some small API medians fluctuate by a
few percent. ABI-heavy byte decoding is also approximately unchanged. The O3
record is a separate paired comparison, not an incremental subtraction from this
table. It verifies the optimized binaries against the same values and errors.

## Bundle sizes and a fixed asset-loading bug

Vite 8.3.7 ES library bundles, oxc minification, gzip level 9. Totals include
all emitted JS chunks and required Wasm. Wasm already embedded as a data URL is
counted inside JS, not a second time as an external file. Maps and unused external
files are excluded. Each optimized variant is rebundled and executed on all four
inputs and a malformed input.

| Deployment | Original, gzip bytes | Source/runtime fixes | Fixes + O3 | Fixes + Oz |
| --- | ---: | ---: | ---: | ---: |
| Official JS adapter | 4,001 | 4,001 | — | — |
| Core JS + Wasm | 14,558 | 14,668 | 11,485 | 11,501 |
| Component JS + Wasm | 53,440 | 33,122 | 28,100 | 28,103 |

The original pinned jco loader used ``new URL(`./${name}`, import.meta.url)``.
Vite expanded it to sibling assets, including JS, declarations, metadata and the
unused standalone Component container. The generated fallback now uses explicit
URLs for the actual extracted core modules. This saves about 38% gzip for the
Component bundle on its own. Source changes plus O3 save about 47% against the
original Component bundle; Core source speed changes add 110 gzip bytes before
postprocessing, while source changes plus O3 are about 21% below the original.

Binaryen 133 is selected explicitly with only the needed Wasm features. No
closed-world, ignored-trap, fast-math or experimental-feature assumptions are used.
The optimized JS-deployed core modules and resident module pass the full
interoperability/malformed-input/ABI-cleanup checks. The original standalone
Component container is retained as provenance; the JS deployment uses extracted
core modules. O3/Oz remain optional postprocessing, not a new default build step.

## SIMD and other experiments

The SIMD candidate packs four i32 values into `v128` and extracts their 16 bytes
to the GC work buffer, with scalar tails. It passes the same correctness checks.
On the numeric-vector encode workload it improved the median by only about
1–2%, while adding 445 bytes to both resident and Core Wasm. The retained version
uses scalar instructions. This result concerns this packing strategy on this
machine, not SIMD in general.

Existing byte storage is a GC array; standard `v128.load` takes a linear-memory
address. Packing values alone does not provide a bulk load/store on that buffer.
See the [Wasm instruction definitions](https://webassembly.github.io/spec/core/syntax/instructions.html#memory-instructions)
and [GC array operations](https://github.com/WebAssembly/gc/blob/main/proposals/gc/Overview.md).
Revisit SIMD when the storage/transfer path can exploit it without extra copies.

Changing only jco's `asyncMode` from `jspi` to `sync` produced byte-identical
143,938-byte JS in the pinned version for this Component. It is not a bundle-size
fix here. The Core/Component boundary and generic task machinery need more
targeted specialization.

## Measurement correction

All prior records are preserved. `compare-scalar/` is an initial paired-v1 pass
with shorter warmup. `compare-scalar-v2/` records a failed longer-warmup run: queued
fulfilled Task Promise jobs accumulated because the loop never returned to the
event loop, eventually reaching Node's heap limit. The failure log and tool source
are retained. This is temporary retention during a synchronous burst, not proof
of a persistent GC leak after the event loop drains.

Final paired-v3 measurements yield the event loop between batches, outside the
sample clock, so fulfilled Task jobs drain. The current general comparison runner
is now `walz-flatbuffers-bench-v2` with the same change and longer warmup. It does
not overwrite or relabel the historical v1 capture. Absolute times from these
different harness versions should not be treated as one before/after series.

The corrected current-v2 benchmark was also run for all four workloads and all
variants with nine samples. It still shows that JS value ABI transfer erases the
resident advantage on these inputs. It is an additional current baseline, not a
rewrite of the original v1 numbers.

Captured `runner.mjs` files are not executed by paired-v3; that tool checks input
and reference hashes itself, and records whether the captured runner hashes match.

## Next targets

1. **Packed mutable byte storage and bulk copies.** Add checked packed i8 array
   support and `array.copy`/range conversion so growth and finish avoid expanded
   i32 work buffers and the intermediate finish array. Preserve snapshot and
   bounds semantics. This targets the largest remaining resident encode share.
2. **Specialize synchronous call bookkeeping.** Avoid creating unused task
   promises/handles on the inline synchronous path while preserving start(),
   cancellation, cleanup, resource and Future-value semantics. Small-call profiles
   and burst retention show why this deserves separate runtime work.
3. **Reduce value ABI traffic.** Keep several parsing/query operations inside
   Wasm and return the requested result in one call. Full record transfers still
   dominate byte-oriented JS calls; test a real application pipeline separately.

## Reproduce

All output directories must be new. Profiles are separate from timing runs.

```sh
just test-flatbuffers
just profile-flatbuffers benchmarks/flatbuffers/20261111-v1/capture target/fb-profile-before
just compare-flatbuffers benchmarks/flatbuffers/20261111-v1/capture benchmarks/flatbuffers/20261111-tuning-v1/after-scalar target/fb-compare
just size-flatbuffers benchmarks/flatbuffers/20261111-tuning-v1/after-scalar target/fb-sizes /path/to/binaryen-133/bin/wasm-opt
just optimize-flatbuffers benchmarks/flatbuffers/20261111-tuning-v1/after-scalar target/fb-O3 /path/to/binaryen-133/bin/wasm-opt
WALZ_FLATBUFFERS_CAPTURE=target/fb-O3 node --expose-gc --test runtime/flatbuffers.test.mjs
```

Browser checks cover both direct and minified bundles using real browser Wasm
loading, plus existing JSPI, Future/Stream and cancellation regressions:

```sh
pnpm exec playwright test runtime/e2e/flatbuffers.spec.mjs runtime/e2e/jspi.spec.mjs
```

Validation passed 178 Node runtime tests, 16 Chromium tests, three native Walz
FlatBuffers tests, the full FlatBuffers checks on the O3 capture, and seven Rust
documentation/example regressions. Raw validation output is saved under
[validation/](validation/).

`size-before/` is the first size pass. Final tables use the paired `size-*-v2/`
records; v2 recognizes literal-URL inlining by hashing the embedded data bytes,
in addition to keeping the old module-attribution evidence. The v1 workload
and official reference were not changed.
