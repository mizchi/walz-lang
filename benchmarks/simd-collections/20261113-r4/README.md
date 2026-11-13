# SIMD collections: scalar, SIMD, and portable batches

Measured 2026-11-12T15:20:38.710Z (2026-11-13 JST), Apple M5 / darwin arm64 / Node v24.22.2 / V8 13.6.233.17-node.63.

All figures are medians of nine samples, in microseconds. Resident construction is excluded; one-shot and batch calls include actual scalar-list ABI copies, packing, guest computation, output conversion, and JS result consumption. Sources and inputs are frozen alongside [report.json](report.json).

## Resident queries

The JS column/range and selected-sum controls include branching and branchless loops; the table uses the faster median in each case. Bitmap JS uses SWAR popcount. The scalar Walz control uses checked GC i32 arrays and scalar Wasm instructions. SIMD uses the packed collection layout; this comparison includes layout effects and does not isolate instruction selection alone.

| Rows / logical bits | Operation | Fastest JS | Scalar Walz | SIMD Walz | SIMD / JS speedup | SIMD / scalar speedup |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 128 | intersection-count | 0.005 | 0.022 | 0.020 | 0.23x | 1.08x |
| 128 | count-between | 0.042 | 0.055 | 0.039 | 1.07x | 1.41x |
| 128 | sum-where | 0.061 | 0.094 | 0.065 | 0.94x | 1.46x |
| 4,096 | intersection-count | 0.120 | 0.080 | 0.043 | 2.78x | 1.85x |
| 4,096 | count-between | 2.274 | 1.484 | 0.832 | 2.73x | 1.78x |
| 4,096 | sum-where | 2.537 | 2.353 | 1.236 | 2.05x | 1.90x |
| 65,536 | intersection-count | 1.932 | 0.911 | 0.377 | 5.12x | 2.42x |
| 65,536 | count-between | 35.927 | 23.596 | 12.889 | 2.79x | 1.83x |
| 65,536 | sum-where | 40.739 | 37.268 | 19.388 | 2.10x | 1.92x |
| 262,144 | intersection-count | 7.543 | 3.556 | 1.442 | 5.23x | 2.47x |
| 262,144 | count-between | 150.447 | 95.838 | 52.833 | 2.85x | 1.81x |
| 262,144 | sum-where | 163.767 | 149.827 | 78.995 | 2.07x | 1.90x |

## One range per transfer

Cost per call, including packing the full column on every call. Faster resident queries do not recover this cost for a single range.

| Rows | Fastest JS | Core + JSPI | Component |
| ---: | ---: | ---: | ---: |
| 128 | 0.083 | 0.578 | 3.592 |
| 4,096 | 2.301 | 4.878 | 8.734 |
| 65,536 | 37.377 | 103.884 | 112.748 |
| 262,144 | 155.478 | 359.527 | 370.547 |

## Thirty-two ranges per transfer

Cost per **whole batch**, not per range. Each call packs the column once and returns all 32 counts; all counts are checked. No persistent host handle or collection table is needed.

| Rows | Fastest JS | Core + JSPI | Component |
| ---: | ---: | ---: | ---: |
| 128 | 2.447 | 2.036 | 5.664 |
| 4,096 | 71.512 | 30.718 | 38.209 |
| 65,536 | 1121.073 | 507.489 | 544.344 |
| 262,144 | 4478.916 | 1983.667 | 2056.250 |

## Interpretation

For 262,144 rows, resident SIMD is 5.23x faster for bitmap intersection, 2.85x for range count, and 2.07x for selected sums relative to the strongest JS control. Do not attribute the full improvement to SIMD because scalar Wasm is also faster than JS.

Single-range portable calls remain slower than branchless JS. Batching 32 ranges reduces 4.479 ms in branchless JS to 1.984 ms with Core + JSPI and 2.056 ms with Component in this run. This is an end-to-end synchronous batch result for the captured dense data, not an arbitrary-query or sparse-data guarantee.

The resident test module is 7,779 bytes (2,789 gzip, 2,502 Brotli), including the resident constructors, exported bitmap/column operations, and scalar controls. These are standalone Core Wasm sizes, not the total portable JS + Wasm bundle size.

## Method and provenance

- Suite version 4: fixed seeded i32 data and bitmap words; all input files use explicit little-endian encoding and have SHA-256 entries. Query limits are retained in query-limits.json.
- Warmup is at least 1,024 calls and 120 ms per variant, or 32 batches and 120 ms for the batch workload. Calibration chooses a common iteration count per comparison near a 20 ms slowest-variant batch. Variants rotate order across samples.
- Explicit GC is outside clocks; automatic GC during clocks counts. No compiler build, test suite, or CPU profiling ran concurrently with measurements. Inputs are bounded integers, so the JS Number sums in these workloads are exact. Wasm sums cross the Core boundary as BigInt; checksum conversion is included.
- Every timed call contributes to a checksum. Correct outputs and all batch result positions are checked outside clocks. Every portable sample checks zero live arena allocations and inspected boundary tables.
- Setup costs for the resident scalar setters and packing are in report.json. They are not included in the resident query table or amortized into its speedups.
- source/ preserves the tested libraries, interfaces, benchmark, and test sources. compiler-sources.json and the report record the tracked compiler inputs, native compiler binary, runtime artifact hashes, and HEAD. HEAD alone does not describe the uncommitted new libraries.
- Earlier r1/r2/r3 runs are retained. R1 had branching JS only; r2 added branchless JS controls; r3 added scalar Walz controls; r4 adds the portable 32-range batch workload and per-result checks. Do not combine samples across suite versions or treat their timings as a controlled implementation speed comparison.

```sh
just test-simd-collections
just bench-simd-collections target/simd-collections-bench/new-run
```

Every output directory must be fresh. Rebuilding measures the current checkout; the frozen files above identify this particular run. The benchmark does not measure browser performance, Worker communication, SIMD on other CPUs, null handling, sparse compression, or arbitrary query selectivities.

## Validation

[Validation logs](validation/) retain the failing initial library/batch checks
and successful checks. The Rust workspace has 704 passing tests and three
existing ignored native-engine tests. The targeted suite passes three Rust
checks (including seven native language tests) and four Node tests with forced
GC and no skips. Core + JSPI and Component each pass the Chromium test,
including the batch API. Four documentation tests and `just check` pass.
All captured source, input, and executed artifact hashes were verified, as was
rejection of an existing output directory without altering its report.
