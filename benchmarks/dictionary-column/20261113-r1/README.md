# Dictionary column: construction, scans, indexes, and value ABI

Measured 2026-11-12T16:33:38.305Z (2026-11-13 JST), Apple M5 / darwin arm64 / Node v24.22.2 / V8 13.6.233.17-node.63.
Suite version 1. This is the primary run; see [the two-run comparison](../README.md). Do not merge their samples.

All figures below are medians of nine samples in **microseconds per whole operation**. Counts use a batch of 32 keys, including four missing keys. Resident inputs are already in each engine. Portable measurements include the actual Core + JSPI / Component value adapters, copies, construction/validation, computation, output conversion, and result consumption.

## 262,144 rows, 64 ASCII labels

| Operation | JS control | Walz | JS / Walz speedup |
| --- | ---: | ---: | ---: |
| Resident encode, JS Map | 2908.111 | 4071.825 | 0.71x |
| Resident supplied-code validation and packing, JS validated copy | 824.387 | 221.963 | 3.71x |
| Resident counts, JS string scans | 21660.250 | 1436.096 | 15.08x |
| Resident counts, JS code scans with retained Map lookup | 3725.408 | 1436.096 | 2.59x |
| Resident counts, scalar Wasm | 2473.042 | 1436.096 | 1.72x |
| Resident counts, retained JS histogram | 1.354 | 1436.096 | 0.000943x |
| Resident category/range query with rows and sum, JS code scan | 228.055 | 212.585 | 1.07x |
| Same query, retained JS postings | 8.072 | 212.585 | 0.04x |

The SIMD count advantage is a **scan** result. Histograms store counts and answer without scanning rows. Postings keep sorted row IDs per category and touch fewer amounts for sparse categories. These are separate algorithm/index choices, not SIMD instructions. The library currently retains neither index. JS histogram/postings construction has separately recorded setup observations in report.json; those single observations are not warmed construction medians. The portable query also times an index built from raw strings on every call.

The small resident row-query difference versus JS code scans is not a robust general advantage; see all raw samples. Four-label data and 1,024-label data have different selectivities. No broad row-query or dictionary-construction win is claimed.

## Count scans across row counts and cardinalities

A speedup below 1x favors JS. The retained-histogram column intentionally exposes a stronger counts-only algorithm. No time in this table is divided by 32.

| Rows-labels-input | JS codes + Map | Scalar Wasm | SIMD Walz | JS scan / SIMD | JS histogram |
| --- | ---: | ---: | ---: | ---: | ---: |
| 4096-4-ascii | 54.325 | 39.842 | 22.737 | 2.39x | 1.257 |
| 4096-64-ascii | 58.100 | 44.059 | 26.617 | 2.18x | 1.336 |
| 4096-1024-ascii | 58.293 | 90.183 | 71.790 | 0.81x | 1.384 |
| 65536-4-ascii | 962.792 | 618.691 | 364.502 | 2.64x | 1.253 |
| 65536-64-ascii | 963.696 | 631.486 | 366.691 | 2.63x | 1.340 |
| 65536-1024-ascii | 947.633 | 682.572 | 410.212 | 2.31x | 1.378 |
| 262144-4-ascii | 3846.183 | 2471.208 | 1437.532 | 2.68x | 1.260 |
| 262144-64-ascii | 3725.408 | 2473.042 | 1436.096 | 2.59x | 1.354 |
| 262144-1024-ascii | 3833.708 | 2531.161 | 1468.561 | 2.61x | 1.368 |
| 65536-64-unicode | 940.627 | 625.612 | 368.332 | 2.55x | 1.365 |

The scalar control scans a checked GC i32 array and uses the same dictionary lookup and output-list API. SIMD uses packed four-lane storage, so SIMD/scalar ratios include storage-layout effects. The small-row/high-cardinality case can lose to JS Map lookup because the published Walz dictionary uses a linear search. UTF-8 and JS string equality agree for these captured well-formed strings.

## Full value ABI

| Operation and JS control | JS | Core + JSPI | Component |
| --- | ---: | ---: | ---: |
| Encode raw strings, JS Map | 3032.357 | 67709.792 | 79870.792 |
| 32 counts from raw strings, JS builds histogram each call | 4635.739 | 70586.291 | 84662.792 |
| 32 counts from raw strings, JS encodes and scans each call | 7303.437 | 70586.291 | 84662.792 |
| 32 counts from supplied codes, JS validates and builds histogram each call | 862.600 | 1880.142 | 1931.921 |
| 32 counts from supplied codes, JS validates/copies and scans each call | 4598.500 | 1880.142 | 1931.921 |
| Category/range query with rows and sum, JS raw strings | 484.973 | 67970.959 | 80854.625 |
| Same query, JS builds dictionary and postings each call | 4678.583 | 67970.959 | 80854.625 |

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
