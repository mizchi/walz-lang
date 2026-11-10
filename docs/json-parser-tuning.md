<a id="jsonパーサーのチューニング"></a>

# JSON parser tuning

On 2026-11-06, using Apple M5 / Node v24.21.0 / V8 13.6, pre-change and post-change binaries were measured in the same process against the [initial comparison](json-parser-bench.md).
No language features or public APIs were added. Original numeric text, duplicate-key order, UTF-8 byte error positions, and the 65,536-byte/128-level limits are preserved.

<a id="採用した変更"></a>

## Adopted changes

- Whitespace and digit scans obtain the length outside the loop and avoid rereading the same byte.
- Ordinary string segments are scanned eight bytes at a time. The whole block's bounds are checked before reading, and scanning stops at the first quote, backslash, or control character. The tail is handled one byte at a time.
- Strings without escapes return a single slice without creating a builder. A builder is created only when the first escape appears.
- bytes slices expand to `array.copy` into a new GC array. Bounds and UTF-8 checks are retained.

<a id="変更前後の結果"></a>

## Before and after

Medians for the parser body with inputs resident in GC, in µs/document.

| Input | Before | After | Speed ratio |
|---|---:|---:|---:|
| small-object | 0.37 | 0.25 | 1.49x |
| records-128 | 53.44 | 33.48 | 1.60x |
| records-512 | 220.39 | 139.80 | 1.58x |
| numbers-4096 | 138.20 | 132.17 | 1.05x |
| wide-object | 133.01 | 88.37 | 1.51x |
| ascii-string | 124.14 | 26.83 | 4.63x |
| unicode-escapes | 19.41 | 19.85 | 0.98x |
| nested-64 | 1.26 | 1.03 | 1.21x |
| whitespace | 54.74 | 26.92 | 2.03x |

Parsing and reserializing 40 KB records including input/output ABI costs changed from 520.10→420.74 µs for Core＋JSPI and 545.44→448.25 µs for Component.
The full API improves by about 1.22–1.24x, compared with 1.58x for the parser body. Numeric arrays and Unicode escapes show little change.
Resident Wasm grew from 12,220→13,174 bytes. This run is separate from the initial comparison; the table uses a pre-change version remeasured in the same process.

Saved artifacts include [all samples, input hashes, and sizes from nine comparisons](../benchmarks/json-parser/20261106-r7-tuning/report.json), [tables for every API](../benchmarks/json-parser/20261106-r7-tuning/report.md), and [sources and results for each trial](../benchmarks/json-parser/20261106-r7-tuning/).

<a id="試して採用しなかった変更"></a>

## Changes tried but not adopted

A version accumulated values in construction-only 32-element chunks and created immutable JSON nodes only once at the end.
In five comparisons against the scalar version, numeric arrays dropped from about 135→112 µs and wide objects from about 87→77 µs.
However, 40 KB record arrays rose from about 141→166 µs. The additional buffers for small arrays and objects made this approach too costly, so it was not adopted.
Destructively reversing already published nodes was also rejected because it would introduce a `reads` effect into query functions. Regression checks ensure `get` / `at` remain callable from pure functions.

For SIMD, a version packed 16 bytes from GC bytes into a register and classified them with `i8x16.eq`, `i8x16.lt_u`, and `i8x16.bitmask`.
A 48 KB ASCII string improved from about 30→24 µs over the scalar version, but 40 KB records with many short keys slowed from about 140→167 µs.
A version checking the first eight bytes with scalar operations before SIMD still took about 141→154 µs on records. The final version does not use SIMD.

The [simdjson design](https://github.com/simdjson/simdjson/blob/master/HACKING.md) indexes structural positions in an initial scan and constructs values afterward.
This experiment only classifies string blocks; it does not implement that entire two-stage parser.
Current GC byte arrays cannot directly use linear-memory `v128.load`, leaving the cost of 16 reads and lane insertions.
Future SIMD work should consider input storage and transfer together.

<a id="測定方法"></a>

## Measurement method

The pre-change `20261106-r2` and post-change `20261106-r7-unroll` were fixed, and the inputs and benchmark Walz were checked for identical SHA-256 hashes.
Input preparation, compilation, instantiation, and output verification are excluded. After three warmup batches, nine measurements rotate execution order.
Explicit GC occurs only before each batch; automatic GC within the batch is timed.
Resident parsing returning the actual GC AST is measured separately from Core＋JSPI / Component parsing status, reserialization, and input-length controls.
After all batches, checks verify AST reserialization hashes, exact equality of normalized strings, and zero outstanding ABI buffers.

Past captures and results are not rewritten. Per-trial binaries and all sources are saved under `target/json-parser-bench/` in `20261106-r3-scalar`, `20261106-r4-chunks`, `20261106-r5-simd`, `20261106-r6-simd-prefix`, and `20261106-r7-unroll`.

```sh
just test-json
just test-json-bench
just json-bench-capture target/json-parser-bench/new-tuning
just json-bench-compare target/json-parser-bench/20261106-r2 \
  target/json-parser-bench/new-tuning target/json-parser-bench/new-comparison 9
```

The comparison implementation is saved alongside the results. Run the saved `compare.mjs` with `node --expose-gc` to repeat the same comparison in another new output directory.

<a id="検査"></a>

## Checks

`just check`, 327 Rust tests, 83 Node tests with GC enabled, and 48 Chromium tests passed.
Checks cover ordinary strings from short inputs to 48 KB, every position at 8/16-byte boundaries, Unicode and escapes, error positions for truncated input/control characters, and all small bytes-slice subranges and out-of-bounds cases.
They observe both original JSON values and the results of `prepend` / `set`, verifying immutability and duplicate-key order too.
