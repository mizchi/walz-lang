# Dictionary-column measurements

Fused dictionary IN/range queries improve resident row-returning queries, while
retained JS indexes and raw-string ABI costs remain important limits. The initial
count-scan benchmark and the subsequent fused-query benchmark are kept separately.
A scan benchmark alone is insufficient evidence for replacing a JS library.

## Fused queries: suite version 2

Two independent processes time the original bitmap composition and fused query
in the same module, with the same input, output, and checksum readers. These
queries return row IDs, count, and exact i64 sum. The query uses one category key
and a signed half-open amount range; inputs are already resident in each engine.

262,144 rows / 64 labels; microseconds per complete query:

| Run | Bitmap composition | Fused | Faster JS code scan | Retained JS postings |
| --- | ---: | ---: | ---: | ---: |
| Primary | 209.691 | 85.985 | 233.081 | 8.643 |
| Confirmation | 209.232 | 84.687 | 223.831 | 9.247 |

Fusion improves on the original by **2.44x / 2.47x**, and on JS code scans by
**2.71x / 2.64x** here. It avoids intermediate bitmaps and skips amount blocks
without a matching category. Improvements over bitmap composition range from
1.11x to 2.95x across the two captured matrices, with a smaller gain for denser
four-label data. Sparse JS postings remain much faster at 64 and 1,024 labels;
the dense four-label case has a different crossover. Count-only APIs are unchanged.

Raw-string Core calls remain about 66 ms and Component calls about 78 ms for this
query. Fusion does not establish an end-to-end speedup there: string conversion,
encoding, and packing still dominate. No new persistent handle API or index is
introduced. Both runs finish all 80 comparison groups, with nine samples per
variant and complete output/ABI cleanup checks. Inputs match suite 1 exactly.

The executable source snapshots, compiler binary, inputs, and executed artifacts
match between these runs. The broader inventories differ only in the two SIMD
README files edited during the primary run; that documentation change is recorded
in the [inventory comparison](validation/fusion/comparison-integrity.log).

- [Primary fused run and all ten resident cases](20261113-r3/README.md)
- [Independent confirmation](20261113-r4/README.md)
- [Fusion validation logs](validation/fusion/)

## Before fusion: suite version 1

Two independent processes ran suite version 1 on Apple M5 / Node 24.22.2.
Both completed all 80 comparison groups, with nine samples per variant, complete
output checks, and zero live ABI allocations/tables after portable samples.
The source manifests, compiler binary hashes, inputs, and executed artifacts
match. Each table cell below is that run's median; samples are not pooled.

262,144 rows, 64 ASCII labels; times are **microseconds per operation**. Counts
are batches of 32 queries, including four absent keys.

| Operation / control | Primary JS | Primary Walz | Confirmation JS | Confirmation Walz |
| --- | ---: | ---: | ---: | ---: |
| Resident encode, JS Map | 2908.111 | 4071.825 | 2935.012 | 3985.823 |
| Resident supplied-code validation/packing, JS validated copy | 824.387 | 221.963 | 826.802 | 220.156 |
| Resident counts, JS code scans with retained Map | 3725.408 | 1436.096 | 3748.758 | 1434.676 |
| Resident counts, retained JS histogram | 1.354 | 1436.096 | 1.373 | 1434.676 |
| Resident row query, JS code scan | 228.055 | 212.585 | 216.656 | 210.384 |
| Resident row query, retained JS postings | 8.072 | 212.585 | 7.918 | 210.384 |
| Supplied-code count batch, JS validates and builds histogram | 862.600 | 1880.142 Core / 1931.921 Component | 856.059 | 1845.504 Core / 1891.992 Component |
| Raw-string count batch, JS builds histogram | 4635.739 | 70586.291 Core / 84662.792 Component | 4689.667 | 67452.334 Core / 79208.209 Component |

The resident count scan is 2.59x / 2.61x faster than JS code scans. The equivalent
scalar-Wasm control is 2473.042 / 2480.964 microseconds, so SIMD with packed
storage improves on it by 1.72x / 1.73x. Those are the supported scan claims.

For counts alone, retained JS histograms are over 1,000x faster here because they
avoid rescanning rows. Postings also avoid full scans for sparse categories. Their
construction/storage costs are separate from reused queries; they are different
algorithms and are not retained in the current Walz column. Building a validated
histogram on each supplied-code call still beats the full Walz value-ABI batch.

Resident row queries return all qualifying row IDs, the count, and exact sum.
The small difference against JS code scans varies between runs and is not a
general advantage. Walz loses against retained JS postings. At 4,096 rows and
1,024 labels, even the count scan loses to JS Map lookup: 71.790 vs 58.293
microseconds in the primary run, reproduced at 71.348 vs 59.563.

Use these results to choose a workload before tuning the implementation. Reuse
packed columns inside a guest when repeated scans and bitmap composition are
needed. Do not ship raw string columns across the ABI for each small query and
expect the resident SIMD speedup to carry over. These baseline results motivated
the fused query above. Optional immutable count/row indexes remain future work;
their performance is not established by these measurements.

- [Primary run: inputs, source snapshots, artifacts, and detailed results](20261113-r1/README.md)
- [Confirmation run: independent samples of the same suite](20261113-r2/README.md)
- [Validation logs](validation/)
- [API and performance guide](../../docs/dictionary-column.md)

```sh
just test-dictionary-column
just bench-dictionary-column target/dictionary-bench/new-run
```

Use a fresh output directory. Each run builds before timing. These are synchronous
Node results on one CPU, with resident construction measured separately; they do
not establish browser, Worker, memory-footprint, or other selectivity results.
