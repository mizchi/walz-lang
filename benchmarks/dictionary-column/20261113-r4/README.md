# Fused dictionary IN/range row queries

Measured 2026-11-12T17:04:37.043Z (2026-11-13 JST), Apple M5 / darwin arm64 / Node v24.22.2 / V8 13.6.233.17-node.63.
This is suite version 2, an independent confirmation of the fused-query run.
All figures are medians of nine samples in **microseconds per complete query**. See [the comparison and earlier suite](../README.md); samples are not pooled across processes or versions.

## Paired resident results

Both implementations return every selected row ID in ascending order, the count, and an exact i64 sum. The baseline composes dictionary IN and numeric range bitmaps, AND, count, sum-where, and indices. The fused path resolves the key once and scans packed code blocks, skips amount blocks with no matching code, applies the range mask, widens before summing, and emits rows without intermediate bitmaps. Its private row buffer grows with the result size; it retains no input column.

Construction and input transfer are excluded from this resident table and measured separately. The bitmap and fused implementations are timed in the **same process and module**, with the same result wrapper/checksum readers. Do not infer the improvement by subtracting timings from suite 1.

| Rows-labels-input | Bitmap composition | Fused | Baseline / fused | Faster JS code scan | Retained JS postings |
| --- | ---: | ---: | ---: | ---: | ---: |
| 4096-4-ascii | 3.702 | 2.120 | 1.75x | 4.077 | 1.492 |
| 4096-64-ascii | 3.401 | 1.229 | 2.77x | 3.393 | 0.143 |
| 4096-1024-ascii | 3.394 | 1.202 | 2.82x | 2.956 | 0.062 |
| 65536-4-ascii | 57.532 | 38.713 | 1.49x | 68.914 | 46.574 |
| 65536-64-ascii | 52.828 | 20.663 | 2.56x | 57.255 | 1.742 |
| 65536-1024-ascii | 52.669 | 17.830 | 2.95x | 46.555 | 0.136 |
| 262144-4-ascii | 228.311 | 183.423 | 1.24x | 270.918 | 228.285 |
| 262144-64-ascii | 209.232 | 84.687 | 2.47x | 223.831 | 9.247 |
| 262144-1024-ascii | 208.382 | 70.755 | 2.95x | 186.651 | 0.480 |
| 65536-64-unicode | 52.877 | 20.703 | 2.55x | 55.866 | 1.724 |

For 262,144 rows / 64 labels, fusion improves 209.232 to 84.687 microseconds (2.47x). It beats the faster JS code scan by 2.64x, but retained JS postings take 9.247 microseconds. The four-label case has denser candidates and a smaller fusion improvement. Do not claim a universal improvement over indexes or arbitrary selectivities.

These row queries have **one present category key**, category zero (the first dictionary entry), and a signed half-open amount range [-500,500). Categories are approximately uniform after the first occurrence of each key. The API's multi-key, duplicate/missing-key, empty-input, overflow, and length-trap behavior is tested separately; those cases are not performance claims in this query table. Count-only batches still contain 32 queries and are unchanged by this optimization.

## Complete raw-string value ABI

262,144 rows / 64 labels, with full input conversion, packing/encoding, query, output conversion, and result consumption on every call:

| Backend/control | Bitmap composition | Fused |
| --- | ---: | ---: |
| Core + JSPI | 65590.208 | 65447.125 |
| Component | 78327.292 | 78396.750 |

JS raw-string scanning takes 395.311 microseconds. JS building a dictionary and postings on every call takes 4488.000 microseconds. Raw-string ABI costs dominate the Walz calls; this suite does **not** establish an end-to-end gain for that API. Resident GC-reference exports are measurement tools, not a stable Component or Worker handle API.

## Suite and validation

Suite 2 retains all ten inputs and 80 comparison groups from suite 1, adding walzFused beside the resident composed query and coreComposed/componentComposed beside the portable fused query. The scalar Wasm count control, JS branching/branchless scans, retained histogram, retained postings, and per-call index-building controls remain. All count, construction, and portable samples are in [report.json](report.json).

- Warmup: at least eight calls and 120 ms per variant. Variant-specific calibration targets a 20 ms sample; variants rotate across nine rounds. Raw samples and exact iteration counts are retained.
- Four forced GC cycles occur outside each sample's clock. Automatic GC during timing counts. All builds finish before timing; this task ran no builds, tests, or CPU profiles concurrently with measured samples. Other user applications were left running.
- Every call contributes a checked checksum. Actual last timed outputs are fully checked outside clocks, including all row IDs and sums. The two resident Walz variants use identical exported checksum readers; their cost is included. Sums are bounded exact JS Numbers in this benchmark; guest i64 results cross as BigInt.
- Portable samples check zero live arena allocations and boundary tables. Source/column lifetime is also checked separately by forced-GC reachability tests; a held result or row list must remain usable while source columns collect.
- Native and both JS backends test signed extrema, sums beyond i32, padded zero lanes, missing/duplicate keys, empty/reversed ranges, mismatched lengths even for empty queries, and dense output buffer growth. Checked IR remains available, with no host import, linear-memory dependency, or effect/opacity bypass in the resident module. Chromium functional checks pass for both backends; browser speed is not measured.

## Provenance

Inputs match suite 1 byte-for-byte. Each `inputs/<dataset>/` directory contains vocabulary, range bounds, and explicit little-endian i32 IDs, amounts, and count-query IDs. The first k rows establish all IDs; later rows use high bits of the recorded seeded LCG. Both resident engines reuse vocabulary strings. amounts[0] = 42 is a qualifying positive control.

[source/](source/) freezes the three libraries, two query interfaces, standalone example, runner, JS controls, and native/Node/browser test sources. Executed artifacts are resident.wasm, core/, and component/. The report records their hashes, compiler-binary hash, dirty-checkout HEAD, and all input hashes. HEAD alone does not describe the new libraries.

The two suite-2 runs have identical frozen executable sources, input hashes, compiler binary, and executed artifacts. Their broader source inventories differ only in the English/Japanese SIMD README files edited during the primary run; [the inventory comparison](../validation/fusion/comparison-integrity.log) preserves those hashes. No measured code changed. sourceManifestSha256 hashes the canonical compact JSON object in source-manifest.json, not the pretty-printed file bytes.

[integrity.txt](integrity.txt) records median/hash/input reconstruction checks. [run.log](run.log) preserves console output. [Validation logs](../validation/fusion/) retain Red, passing checks, GC/ABI tests, Chromium checks, and archive verification. Construction and index setup observations remain separate, not amortized into the resident table. JS histogram/postings setup observations are single observations, not warmed construction medians.

```sh
just test-dictionary-column
just bench-dictionary-column target/dictionary-bench/new-run
```

Use a fresh output directory. Rerunning measures the current checkout. This benchmark does not measure threads, Worker communication, browser performance, memory footprint, new count/postings indexes, narrow code packing, other CPUs, or arbitrary key/range distributions.
