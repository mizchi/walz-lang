<a id="簡潔索引とworkerバッチ"></a>

# Succinct indexes and Worker batches

Walz implementations of immutable BitVectors with rank/select and a Wavelet Matrix over u8 values. The base index is a u32 prefix every 512 bits. Standalone rank uses scalar operations; rank within Wavelet uses v128 popcount and pairwise addition. select chooses a block with upper_bound over prefixes, then scans at most 16 words. It handles duplicate prefixes for empty blocks and select0 excluding tail padding.

The implementation is in [examples/succinct](../examples/succinct). Construction, validation, and queries are in Walz; Worker management uses the [generic WorkerHost](worker-host.md); index-specific integration lives in a small [driver](../runtime/succinct/driver.mjs). This is an experimental library for repeated range statistics, rather than an API replacing standard arrays or JSON ASTs wholesale.

```sh
just succinct-build target/succinct/library
# 単体APIをComponentにする場合
just succinct-build target/succinct/component component
```

Distribute generated sdk, workers, serial, and reader.wasm together. Development requires native Lean; running generated artifacts requires neither the compiler nor Lean.

```js
import { instantiate, queries } from './target/succinct/library/sdk/index.mjs';
const sdk = await instantiate(new URL('./target/succinct/library/', import.meta.url));

await using index = sdk.buildWavelet(Uint8Array.of(8, 1, 5, 2, 9, 3));
await index.prepare({ workers: 4 });
const frequency = await index.queryMany(
  queries.rangeFreq([[0, 6, 2, 8]]), // [0,6)内の値[2,8)の個数
  { workers: 4 },
); // Int32Array [3]
const quantile = await index.queryMany(
  queries.quantile([[0, 6, 2]]), // 0-originの2番目
  { workers: 4 },
); // Int32Array [3]
```

Construct a BitVector with `buildBits(bytes, logicalBitLength)`. Positions count from each byte's low bit, and unused trailing bits must be 0. `queries.rank1(ends)` counts ones in half-open [0,end); `queries.select1(ranks)` / `select0(ranks)` return zero-based positions. Missing select results and quantiles for empty ranges or out-of-range k return -1. Out-of-range rank, reversed intervals, and invalid value ranges are errors. Wavelet values range from 0 to 255; rangeFreq's upper bound can be 256.

Original data is copied during construction. `snapshot()` is also an independent copy; modifying it does not affect the index. Shared data consists of the index itself and auxiliary tables such as prefixes and zero counts. GC references are not shared. Queries go to Worker-private memory, and results return by transfer from private buffers. Multiple queryMany calls also use the generic host's queue. Result order within each queryMany is preserved, and AbortSignal can cancel just one batch.

The first prepare fixes the Worker count for that index. The batch limit is 524,288 queries. Initial shared memory is fixed at 16 MiB; `byteLength` reports the index's logical size rather than capacity. Private output memory grows with query count. Reducing capacity for small indexes remains future tuning.

Initialization occurs once before publication, after which the [readonly-region contract](readonly-regions.md) applies. close and cancellation wait for reads to complete, avoiding premature reuse of shared regions. No static Builder partial moves or generic borrow checker are added.

<a id="検査と再計測"></a>

## Checks and remeasurement

`just test-succinct` checks an independent oracle over expanded arrays, JS with the same indexing scheme, both standalone backends, 1/2/4 Workers, Chromium, cancellation and shutdown failures, region guards, and Lean certificates. Cases include empty inputs, all-zero/all-one data, sparse sequences, 32/512-bit boundaries, missing selects, frequencies, and quantiles.

```sh
just bench-succinct new-run-id ../jsimd
node --expose-gc runtime/succinct/compare.mjs before-id after-id new-comparison-id
```

Measurements accept only new IDs. They save Walz sources, actual jsimd public APIs and Wasm, artifacts, inputs, compiler hashes, Node/V8/CPU, and nine samples. JS using the same index serves as a control; algorithmic differences between jsimd's linear select and indexed select are not presented as Walz-specific speedups.

The [initial scalar version](../benchmarks/succinct/20261108-r1-scalar/report.md), [SIMD trial](../benchmarks/succinct/20261108-r2-simd/report.md), [same-process alternating comparison](../benchmarks/succinct/20261108-r3-comparison/report.md), and [generic-host version](../benchmarks/succinct/20261108-r4-general-host/report.md) are saved separately.

In the alternating comparison, SIMD improved rangeFreq by about 1.30x and quantile by about 1.06x, but made standalone rank about 9% slower, so standalone rank retains scalar operations. For 262,144 elements and 32,768 queries in the generic-host version, quantile took 11.10 ms for same-scheme JS, 6.16 ms for jsimd, 6.14 ms for Walz local, and 1.69 ms for Walz with four Workers. rangeFreq took 18.90 ms for JS, 13.49 ms for jsimd, 11.11 ms for Walz local, and 2.92 ms for four Workers.

These are batch medians excluding construction/startup but including query copies, result retrieval, and Worker communication. JS is faster for small standalone rank queries. Worker preparation took roughly ten to several tens of milliseconds in this run and must be amortized over multiple batches with resident Workers. build/prepare/first query are recorded separately; break-even batch counts are estimates from that one setup measurement and steady-state medians. They are not full cold-start distributions or measured p95 values.

Conditions were Apple M5, Node 24.21.0/V8 13.6, and jsimd `2cf51e8`. Wavelet across the full u8 range uses about 1.063 bytes/value; no claim is made that it is smaller than Uint8Array. This implementation uses fixed-ratio auxiliary tables and does not prove mathematical n+o(n) or entropy bounds.
