<a id="simd-カーネルと速度の比較"></a>

# SIMD kernels and speed comparisons

Four operations from the [jsimd README](https://github.com/mizchi/jsimd/blob/main/packages/jsimd/README.md) are implemented in Walz: i32 aggregation, Float32 dot product, bitmap intersection count, and bytes search. These examples investigate whether SIMD over GC can reach practical speeds; they do not port entire data structures or every method.

| Operation | Walz functions | Value conventions |
| --- | --- | --- |
| i32 sum | `sum-scalar` / `sum-simd` | Extend signed i32 to i64 before addition, avoiding i32 overflow |
| Float32 dot product | `dot-scalar` / `dot-simd` | Equal lengths. Scalar uses sequential f32 additions; SIMD accumulates into four lanes and reduces them |
| Bitmap intersection count | `bitmap-scalar` / `bitmap-simd` | Equal-length sequences of 32-bit words; popcount of ANDed bits |
| Byte search | `bytes-scalar` / `bytes-simd` | First matching position, or −1 if absent; needle from 0 to 255 |

<a id="実装と使い方"></a>

## Implementation and usage

[kernels.walz](../examples/simd-kernels/kernels.walz) uses ordinary GC arrays and `array v128`, without linear memory. `pack-i32` / `pack-f32` pack four elements at a time; `pack-bytes` packs 16 bytes, padding incomplete tails with zero. Search retains the logical length so padding zeros are not returned as matches. Packed results are snapshots independent of the source scalar array.

Integer, float, and byte lanes have separate nominal types. Internal mutable GC arrays are opaque, preventing Walz callers from manipulating their representations. Only the type/effect/ownership-checked core is used, emitting SIMD instructions directly. No language syntax or runtime is added. Required standard Wasm `i32/i64.popcnt` / `ctz` / `clz` instructions are also handled by the typed core. Law models and range refinements for these are not added.

For use within Walz, see [demo.walz](../examples/simd-kernels/demo.walz). `just run examples/simd-kernels/demo.walz` returns 42. Repack after modifying arrays; retain packed results when processing the same data repeatedly.

[portable.walz](../examples/simd-kernels/portable.walz) accepts list/bytes, computes with scalar or SIMD operations, and returns values. It supports both Core＋JSPI and Component output.

```sh
just build-core-js examples/simd-kernels/portable.walz target/bulk
```

```js
const { instantiate } = await import("./target/bulk/index.js");
const { api } = await instantiate();
api.sumSimd(new Int32Array([10, 20, 12]));             // 42n
api.dotSimd(new Float32Array([1, 2, 3, 4, 5]),
  new Float32Array([2, 4, 6, 8, 10]));                // 110
api.bitmapSimd(new Int32Array([-1, 15, 0, 8, 1]),
  new Int32Array([3, 7, 9, 8, 1]));                   // 7
api.indexSimd(new Uint8Array([1, 2, 255]), 255);      // 2
```

`sum` returns i64, hence BigInt in JS. Float32 rounding and addition order follow Wasm semantics and generally differ from sequential JavaScript double addition. Length mismatches and invalid needles trap. Do not reuse Component instances after traps; for recoverable failures, validate before calling or use a Result-returning wrapper.

<a id="測定方法"></a>

## Measurement method

The following costs are measured separately.

- **resident**: Construct inputs once in GC arrays and retain packed values. Includes JS calls and result reception; excludes initialization, packing, and WIT conversion.
- **pack + compute**: Pack from existing scalar GC arrays on every call and compute. Includes GC allocation and collection.
- **including copies**: Includes JS TypedArrays, actual Core＋JSPI/Component value adapters, GC-array conversion, packing, computation, and ABI-buffer release. Excludes module loading and compilation.

The resident comparison's [resident.walz](../examples/simd-kernels/resident.walz) returns Core Wasm GC references as opaque JS objects and passes them back to the same instance. Initial data is transferred with setters, excluding that cost from resident measurements. This is a Core interface separate from the WIT value ABI. `v128` itself is not returned to JS. The scalar byte reference implementation uses GC i32 arrays, whereas SIMD packs bytes into 128 bits, so memory-density differences are included too.

The actual generated Wasm from a local jsimd checkout is saved for comparison. Resident measurements use raw linear-memory kernels without the public JS wrapper; copy-inclusive measurements sum `TypedArray.set` into preallocated scratch and kernel time. Public-library allocation and argument checks are excluded. In particular, jsimd's actual bytes API handles small inputs and leading matches in JS, so these comparisons alone do not establish full public API performance.

JS references use TypedArray for loops, SWAR popcount, and `Uint8Array.indexOf`. Float32 benchmark inputs are small integers within a range unaffected by addition order. Correctness with fractional rounding is checked separately in runtime tests.

All implementations' answers are checked for the same inputs. After warmup and iteration calibration, execution order rotates, and medians/minima/maxima over nine samples are saved. Sizes are 16, 1,024, 4,096, 16,384, and 262,144 elements. Bytes searches separate misses, final matches, and initial matches. Measurements reflect this environment, including bandwidth, caches, JIT, and GC.

<a id="測定結果2026-11-06"></a>

## Results (2026-11-06)

Apple M5, Node v24.21.0 / V8 13.6. Medians over nine samples, in µs/call. The reference saves generated binaries from a local jsimd checkout at commit `2cf51e8`, hashing sources and binaries separately.

With 16,384 resident elements:

| Operation | JS | Walz scalar | Walz SIMD | jsimd kernel | Walz SIMD speedup over JS |
| --- | ---: | ---: | ---: | ---: | ---: |
| i32 sum | 8.719 | 5.558 | 1.902 | 1.892 | 4.58x |
| Float32 dot product | 10.349 | 10.182 | 2.537 | 2.536 | 4.08x |
| Bitmap intersection count | 16.000 | 7.196 | 2.878 | 6.001 | 5.56x |
| Bytes search, miss | 3.786 | 4.955 | 0.534 | 0.458 | 7.09x |

Resident-vector kernels become sufficiently fast even over GC. i32 sum and dot product nearly matched the reference kernels. At 262,144 elements, they were still 3.76–7.32x faster than JS. However, some operations favored JS at 16 elements, and JS remained faster for initial bytes matches regardless of size.

Passing the same 16,384 elements from JS on every call:

| Operation | JS | Core＋JSPI SIMD | Component SIMD | jsimd scratch copy＋kernel |
| --- | ---: | ---: | ---: | ---: |
| i32 sum | 8.719 | 561.776 | 23.695 | 2.742 |
| Float32 dot product | 10.349 | 1136.948 | 42.821 | 4.891 |
| Bitmap intersection count | 16.000 | 1114.953 | 43.416 | 7.746 |
| Bytes search, miss | 3.786 | 17.875 | 20.166 | 0.604 |

Current portable one-shot calls are slower than JS even with SIMD. Merely repacking existing GC arrays on every call took 11.126 µs for dot product and 5.581 µs for bytes search, both longer than JS. Retaining packed data for repeated use is necessary to benefit from SIMD.

In these initial measurements, Core＋JSPI list inputs passed each element through a value codec and DataView. Scalar and SIMD times were nearly equal, and substantially different from Component using the same kernels, motivating improvements to boundary transfer.

Core Wasm combining scalar/SIMD versions of the four operations and resident constructors was 3,456 bytes (1,391 bytes gzip). The portable version differs in size because it includes WIT/ABI and GC conversion. This is not a direct bundle-size comparison against jsimd's per-feature modules.

All sizes, raw samples, and input hashes are saved in [report.json](../benchmarks/simd-kernels/20261106-r1/report.json), readable tables in [report.md](../benchmarks/simd-kernels/20261106-r1/report.md), and artifact/source hashes in [capture.json](../benchmarks/simd-kernels/20261106-r1/capture.json). The original executable snapshot is at `target/simd-kernels/20261106-r1`.

<a id="数値listの一括転送後2026-11-06"></a>

## After bulk numeric-list transfers (2026-11-06)

Core＋JSPI now bulk-copies numeric list inputs from matching TypedArrays and numeric list outputs. Ordinary Array value validation, list limits and bounds, and release rules remain intact. See the [list ABI](lists.md).

Saved pre-change and post-change outputs were measured alternately, with nine samples each. Hashes verify identical Wasm binaries, common host runtimes, inputs, and measurement functions, isolating generated JS-side type-conversion changes. The following medians cover 16,384 elements, including input copies, GC conversion, packing, SIMD computation, and cleanup, in µs.

| Operation | Core before | Core after | Speed ratio | Component after (control) |
| --- | ---: | ---: | ---: | ---: |
| i32 sum | 612.414 | 22.187 | 27.60x | 25.023 |
| Float32 dot product | 1122.943 | 41.715 | 26.92x | 45.199 |
| Bitmap intersection count | 1221.011 | 42.870 | 28.48x | 45.482 |
| Bytes search, miss (control) | 18.309 | 18.199 | 1.01x | 21.384 |

The three TypedArray-input operations improved by 23–26x at 4,096 elements and 19–21x at 262,144 elements. Ordinary Array paths retain per-element checks and take similar time to before. bytes and Component are unchanged. Large inputs also show variability in the GC-allocating controls; each sample's minimum and maximum are recorded.

This improves transfers between JS and linear memory. GC conversion and packing remain, so these one-shot calls are still slower than JS scalar loops. Next targets are reducing intermediate GC copies and reusing packed data.

Raw samples, input hashes, comparison code, and pre/post-change manifests are saved in the [comparison results](../benchmarks/simd-kernels/20261106-r2-bulk/report.md) and [report.json](../benchmarks/simd-kernels/20261106-r2-bulk/report.json). The initial r1 remains unchanged.

<a id="再実行"></a>

## Rerunning

```sh
just test-simd-kernels
pnpm exec playwright test runtime/e2e/simd-kernels.spec.mjs
just simd-kernels-capture target/simd-kernels/my-run ../jsimd
just simd-kernels-bench target/simd-kernels/my-run
# 対応するTypedArrayを使い、保存した変更前後の型変換を交互に測る。
just simd-kernels-compare target/simd-kernels/20261106-r1 \
  target/simd-kernels/20261106-r2-bulk target/simd-kernels/my-comparison
```

Build the reference `kernels.wasm` in jsimd first. capture does not overwrite existing directories; it saves Walz sources/output, jsimd binaries/sources, and measurement code with hashes. run executes the saved measurement code and rejects detected modifications. Each run saves new results at `results/<timestamp>/report.json` / `report.md`. It also records input seeds/content hashes, Node/V8/CPU, the reference checkout commit, and artifact sizes.
