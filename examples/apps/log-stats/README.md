# Streaming Log Aggregation

English | [日本語](README.ja.md)

Reads logs from a file or standard input and outputs JSON containing count, total duration, and maximum duration for each level.
Parsing, aggregation, integer formatting, and JSON construction are all implemented in walz. The host only connects existing text I/O capabilities and performs no aggregation.
This uses bytes.from-array, limited Result/Stream macros, and opaque types with the existing ABI and task runtime.

Run with `walz-lang` as the working directory.

```sh
just build-log-stats
just log-stats                         # sample.tsv を集計
just log-stats /path/to/input.tsv
node runtime/text-cli.mjs target/log-stats < input.tsv > report.json
node runtime/text-cli.mjs target/log-stats --input input.tsv --output report.json
just test-log-stats
just analyze examples/apps/log-stats/main.walz
just profile-log-stats
```

## Input/Output Contract

Each input line is `LEVEL<TAB>duration_ms<TAB>message`.

- LEVEL is uppercase INFO/WARN/ERROR.
- Duration_ms is an ASCII decimal integer from 0..4294967295. Empty values, signs, whitespace, and overflow are rejected; leading zeros are allowed.
- A second TAB is required. Message may be empty; any later TAB is part of its contents. Message contents are not retained in aggregation state.
- Empty lines and lines beginning with `#` count as ignored. Other invalid syntax counts as rejected, and processing continues.
- The existing I/O layer handles LF/CRLF, a final line without a newline, and split UTF-8 sequences. The default line-length limit is 1 MiB.

See [expected.json](expected.json) for the sample output. Empty input uses the same schema with all counters at zero.
The string library constructs the entire report as one JSON line.
For formatted output, use a command such as `just log-stats | jq .`.

Counts and totals use unsigned 64-bit arithmetic. A record that overflows leaves state unchanged and terminates the whole operation with `err(1001)`.
JSON integers use decimal notation. Ordinary JavaScript `JSON.parse` does not guarantee integer precision above 2^53-1.
Invalid UTF-8 and I/O errors terminate through existing Result errors. The report is produced only if aggregation reaches normal EOF.
Failure during output can leave a partial report; use the CLI exit code to determine success.

## Structure and State

Processing moved from the initial five files/about 260 lines into shared libraries and limited macros.

| File | Responsibility | Effect bound |
|---|---|---|
| [parse.walz](parse.walz) | Parses TSV/u32 and returns skip/invalid/info/warn/error | allocates |
| [stats.walz](stats.walz) / [stats-impl.walz](stats-impl.walz) | Opaque state, counts/totals/maxima, and overflow checks | allocates during initialization; reads/writes for updates |
| [decimal.walz](decimal.walz) | Connects to shared u64 conversion | reads / writes / allocates |
| [report.walz](report.walz) | JSON construction and one asynchronous write | reads / writes / allocates / host / suspends |
| [main.walz](main.walz) | Iteration and error propagation with fold-result/result.let | reads / writes / allocates / host / suspends |
| [walz/core/text.walz](../../../walz/core/text.walz) | Concatenation, signed/unsigned integers, and JSON strings | reads / writes / allocates |

Writes in string operations conservatively include changes to working arrays created inside the function; input text/bytes stays immutable.
Aggregation-state field operations are allowed only in the file declaring the type.
The public Level enum is info/warn/error and does not accept arbitrary integers as array indices would.
Input-line enums directly contain level and duration, avoiding a separate Level object for every line.

Static linking uses @use/@export, without dynamic function lookup or module objects.
Aggregation-state size is fixed by the level count and does not grow with input line count. The Wasm engine reclaims GC objects.
Stream ownership is checked through expanded loop/move/match forms, closing on EOF, input-error, and aggregation-error paths.
The whole JSON write is awaited; on cancellation, the existing task scope completes pending reads/writes and release.

## Tests and Measurements

Seven checks in [model.test.walz](model.test.walz) cover levels, empty lines, missing fields, integer boundaries, independent buckets, aggregation overflow and update atomicity, and decimal conversion of the u64 maximum.
Rust tests run these checks and verify effect boundaries between parsing, aggregation, and I/O.
Node checks both backends with one-byte UTF-8 input, 25,000-line aggregation, empty input, I/O errors, cancellation while waiting, and asynchronous release.
After completion, endpoint registrations and ABI buffers are zero; linear memory is 64 KiB under these workloads. Chromium runs both backends too.

`just profile-log-stats [output] [rows] [native-rounds]` measures these separately:

- [benchmark.walz](benchmark.walz) calls the same parser and aggregation modules with six input patterns. It verifies result checksums and collects Wasmtime `--profile=guest,...,1ms` samples. This is a separate Wasm binary without the I/O adapter or actual CLI.
- Actual `main.walz` is compiled to Core+JSPI, receiving the same line data in 4 KiB chunks. After three warm-ups, seven runs measure time until main completes. Compilation/instantiation are excluded, and the entire JSON output is compared against independent expectations. Read counters are collected separately after timing.

2026-11-04, Apple M5 / macOS arm64, Node v24.21.0, Wasmtime 42.0.1.
The final Core+JSPI version processed 30,000 lines/560,000 bytes with a median of 25.58 ms, 30,002 JSPI reads, and one output.
At completion, ABI buffers were zero and linear memory was 65,536 bytes. Native execution of 30,000 lines × 100 rounds yielded 312 samples.
Self samples were about 55% stats.consume, 24% parse.fields, and 9% parse.tab. Interrupt-point bias means these are not exact CPU times or a JSPI breakdown.

### Alternating Comparisons Under Different Conditions

```sh
just profile-log-stats target/profile/log-stats-final
just compare-log-stats target/profile/log-stats-before-12346 target/profile/log-stats-final
```

The saved initial version, version after changes 1–4, and final version each received four warm-ups in the same Node process, followed by nine measurements in alternating order.
Compilation/instantiation are excluded; independent ABI-allocation probes run after timing.
Every run checks the entire JSON against independent expectations. Guest hashes and use of the same allocator by probes are checked too.

| Condition | Lines | Initial | After 1–4 | Final |
|---|---:|---:|---:|---:|
| ASCII 32 bytes, no invalid lines, 4 KiB chunks | 16,000 | 13.05 ms | 12.95 ms | 12.80 ms |
| Japanese + emoji, 25% invalid lines, 4 KiB chunks | 16,000 | 16.20 ms | 16.03 ms | 16.23 ms |
| ASCII 4 KiB, no invalid lines, 4 KiB chunks | 1,000 | 5.27 ms | 5.32 ms | 5.31 ms |
| ASCII 64 bytes, 50% invalid lines, 64-byte chunks | 16,000 | 22.87 ms | 22.65 ms | 22.27 ms |

This comparison shows roughly equivalent performance to improvements of a few percent, not a major speedup. File-I/O throughput was not measured.
Optimizations after changes 1–4 combine input-line enums, directly reference aggregation buckets, and reuse pre-update values.

For each 16,000-line condition, ABI allocations changed from 32,070→32,004, JSPI reads from 16,035→16,002, and outputs from 34→1.
For the 1,000-line condition, ABI allocations changed from 2,070→2,004. All conditions/versions used 64 KiB linear memory and had zero live blocks at completion.
These allocation counts cover only linear ABI buffers, not Wasm GC allocations.

Artifacts are in `target/profile/log-stats-before-12346/`, `log-stats-after-1234/`, and `log-stats-final/`.
The table's three-version comparison is saved as `comparison-12346.json`. `comparison.json` contains raw timing/probes from the last comparison command; `report.json` contains standalone native/JSPI measurements.
`guest.json`, executable Core+JSPI artifacts, compute.wasm, and sources are also saved.

## Remaining Gaps

Concatenation, integer conversion, and JSON strings moved into the standard library. Group-by with arbitrary keys, a generic JSON serializer, a general builder, and WASI file bindings remain.
Result/Stream macros preserve ordinary type, effect, and ownership checks. They have not been extended to finally handling arbitrary exceptions or delivery of owned payloads.
Monomorphized generics were not included in this change.
