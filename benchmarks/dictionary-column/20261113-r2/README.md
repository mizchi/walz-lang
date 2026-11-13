# Dictionary column: construction, scans, indexes, and value ABI

Measured 2026-11-12T16:36:59.489Z (2026-11-13 JST), Apple M5 / darwin arm64 / Node v24.22.2 / V8 13.6.233.17-node.63.
Suite version 1. This is an independent confirmation run of the same inputs, sources, and executed artifacts; see [the two-run comparison](../README.md). Do not merge their samples.

All figures below are medians of nine samples in **microseconds per whole operation**. Counts use a batch of 32 keys, including four missing keys. Resident inputs are already in each engine. Portable measurements include the actual Core + JSPI / Component value adapters, copies, construction/validation, computation, output conversion, and result consumption.

## 262,144 rows, 64 ASCII labels

| Operation | JS control | Walz | JS / Walz speedup |
| --- | ---: | ---: | ---: |
| Resident encode, JS Map | 2935.012 | 3985.823 | 0.74x |
| Resident supplied-code validation and packing, JS validated copy | 826.802 | 220.156 | 3.76x |
| Resident counts, JS string scans | 20223.292 | 1434.676 | 14.10x |
| Resident counts, JS code scans with retained Map lookup | 3748.758 | 1434.676 | 2.61x |
| Resident counts, scalar Wasm | 2480.964 | 1434.676 | 1.73x |
| Resident counts, retained JS histogram | 1.373 | 1434.676 | 0.000957x |
| Resident category/range query with rows and sum, JS code scan | 216.656 | 210.384 | 1.03x |
| Same query, retained JS postings | 7.918 | 210.384 | 0.04x |

The SIMD count advantage is a **scan** result. Histograms store counts and answer without scanning rows. Postings keep sorted row IDs per category and touch fewer amounts for sparse categories. These are separate algorithm/index choices, not SIMD instructions. The library currently retains neither index. JS histogram/postings construction has separately recorded setup observations in report.json; those single observations are not warmed construction medians. The portable query also times an index built from raw strings on every call.

The small resident row-query difference versus JS code scans is not a robust general advantage; see all raw samples. Four-label data and 1,024-label data have different selectivities. No broad row-query or dictionary-construction win is claimed.

## Count scans across row counts and cardinalities

A speedup below 1x favors JS. The retained-histogram column intentionally exposes a stronger counts-only algorithm. No time in this table is divided by 32.

| Rows-labels-input | JS codes + Map | Scalar Wasm | SIMD Walz | JS scan / SIMD | JS histogram |
| --- | ---: | ---: | ---: | ---: | ---: |
| 4096-4-ascii | 55.421 | 40.204 | 22.876 | 2.42x | 1.321 |
| 4096-64-ascii | 57.699 | 44.174 | 26.876 | 2.15x | 1.403 |
| 4096-1024-ascii | 59.563 | 89.030 | 71.348 | 0.83x | 1.336 |
| 65536-4-ascii | 919.341 | 620.882 | 362.992 | 2.53x | 1.315 |
| 65536-64-ascii | 954.492 | 629.427 | 369.289 | 2.58x | 1.408 |
| 65536-1024-ascii | 980.447 | 697.438 | 416.476 | 2.35x | 1.390 |
| 262144-4-ascii | 3718.733 | 2464.208 | 1433.798 | 2.59x | 1.316 |
| 262144-64-ascii | 3748.758 | 2480.964 | 1434.676 | 2.61x | 1.373 |
| 262144-1024-ascii | 3693.542 | 2510.066 | 1472.644 | 2.51x | 1.329 |
| 65536-64-unicode | 921.686 | 625.064 | 367.011 | 2.51x | 1.391 |

The scalar control scans a checked GC i32 array and uses the same dictionary lookup and output-list API. SIMD uses packed four-lane storage, so SIMD/scalar ratios include storage-layout effects. The small-row/high-cardinality case can lose to JS Map lookup because the published Walz dictionary uses a linear search. UTF-8 and JS string equality agree for these captured well-formed strings.

## Full value ABI

| Operation and JS control | JS | Core + JSPI | Component |
| --- | ---: | ---: | ---: |
| Encode raw strings, JS Map | 2983.911 | 67097.333 | 78010.875 |
| 32 counts from raw strings, JS builds histogram each call | 4689.667 | 67452.334 | 79208.209 |
| 32 counts from raw strings, JS encodes and scans each call | 6762.972 | 67452.334 | 79208.209 |
| 32 counts from supplied codes, JS validates and builds histogram each call | 856.059 | 1845.504 | 1891.992 |
| 32 counts from supplied codes, JS validates/copies and scans each call | 4514.511 | 1845.504 | 1891.992 |
| Category/range query with rows and sum, JS raw strings | 457.067 | 66259.375 | 77579.833 |
| Same query, JS builds dictionary and postings each call | 4696.625 | 66259.375 | 77579.833 |

Raw-string portable calls are substantially slower than JS here. Supplied-code batches avoid per-row string reconstruction and beat JS validated **scans**, but still lose to the JS validated histogram. The resident GC-reference fixture does not provide a stable portable persistent-handle API; no new handle table or runtime change was made for this benchmark.

## Workloads and measurement contract

- Ten data sets: 4,096 / 65,536 / 262,144 rows crossed with 4 / 64 / 1,024 labels, plus 65,536 rows / 64 Unicode labels. Eight comparison groups per data set, 80 groups in total. Inputs are immutable during queries.
- First occurrence establishes each dictionary ID. The first k rows contain IDs 0 through k-1. Later rows use high bits of a seeded LCG. Both resident engines reuse vocabulary string objects; the portable APIs pay their real string conversion costs. ASCII labels are category-N; Unicode labels are 地域🦀-N.
- Amounts lie in [-4096,4095], with amounts[0] = 42 as a qualifying positive control. Queries use category zero (the first dictionary entry) and the half-open range [-500,500), returning every matching row ID in order, the count, and the exact sum. Count-batch keys are (i*31)%k except every eighth key is absent. No Unicode normalization or malformed surrogate input is included in the timing domain.
- Warmup is at least eight calls and 120 ms per variant. Calibration chooses **variant-specific** iteration counts targeting a 20 ms sample; a common count would badly overrun the slower scans when compared with constant-time index queries. Variant order rotates across nine rounds. Iterations, warmup counts, and all individual samples are in report.json.
- Four forced GC cycles occur before each sample, outside clocks. Automatic GC inside clocks counts. This task ran no builds, test suite, or CPU profiler concurrently with timing; builds finish before measurement begins. Other user applications were not stopped.
- Each timed call contributes a checked checksum. Every actual last timed output is fully verified outside the clock: all dictionary strings and codes, all 32 counts, and every returned row/count/sum. The resident checksum includes small exported-reader calls; JS consumes outputs directly. Wasm i64 sums return BigInt, converted in the checksum; all captured sums are exactly representable as JS Number.
- Portable samples check zero live arena allocations and inspected boundary tables. Generated adapters are instrumented only to expose inspection methods; inspection is outside clocks. These checks do not substitute for the separate forced-GC reachability tests in just test-dictionary-column.
- Resident encode and supplied-code construction have their own warmed medians. Input transfer/freezing, first encode, and JS index setup are separate, unamortized observations in report.setup. Resident query numbers exclude them; they cannot be treated as full application costs.

## Provenance and reproduction

[report.json](report.json) records the dirty-checkout HEAD, engine versions, source hashes, compiler-binary hash, artifact hashes, and input hashes. HEAD alone does not describe these newly added libraries. [source/](source/) freezes the three libraries, the resident and portable interfaces, the benchmark, its JS controls, and control tests. source-manifest.json hashes compiler/runtime/core inputs; sourceManifestSha256 is the digest of its canonical compact JSON object, not its pretty-printed file bytes. Generated executables are retained in resident.wasm, core/, and component/.

Each `inputs/<dataset>/` directory has a vocabulary and predicate in data.json plus explicit little-endian i32 IDs, amounts, and count-query IDs. Reconstruct string rows as vocabulary[ids[i]]. [integrity.txt](integrity.txt) records verified hashes, medians, and reconstruction. [run.log](run.log) retains the console output. [Shared validation logs](../validation/) cover the new benchmark task and its controls.

```sh
just test-dictionary-column
just bench-dictionary-column target/dictionary-bench/new-run
# Optional smaller matrix: output directory, comma-separated row counts, label counts
node --expose-gc runtime/dictionary-column-bench.mjs target/dictionary-bench/small 4096 4,64
```

Output directories must be fresh; the runner rejects an existing directory before building or measuring. A new run measures the current checkout. It does not overwrite or silently reinterpret these recorded measurements. This suite does not measure browser speed, Workers, threads, memory footprint, nullability, packed u8/u16 codes, cold string hashing, other CPUs, or arbitrary predicate distributions.
