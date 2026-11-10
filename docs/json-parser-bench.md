<a id="jsonパーサーの速度比較"></a>

# JSON parser speed comparison

Measured with Node v24.21.0 / V8 13.6 / Apple M5 arm64, Rust 1.98.0, and a release build of serde_json 1.0.154.
Nine fixed inputs were measured nine times, rotating execution order by one each time. Values below are medians per document, in µs.

<a id="入力が既に各処理系にある状態での解析"></a>

## Parsing with inputs already resident in each runtime

| Input | UTF-8 bytes | JSON.parse | serde_json | Walz / Wasm GC | Walz time / Node |
|---|---:|---:|---:|---:|---:|
| small-object | 56 | 0.16 | 0.19 | 0.37 | 2.27x |
| records-128 | 9,633 | 14.81 | 26.20 | 52.74 | 3.56x |
| records-512 | 40,161 | 58.91 | 106.86 | 217.48 | 3.69x |
| numbers-4096 | 25,747 | 40.47 | 47.70 | 132.36 | 3.27x |
| wide-object | 24,891 | 121.22 | 181.06 | 129.89 | 1.07x |
| ascii-string | 48,002 | 12.09 | 3.17 | 128.74 | 10.65x |
| unicode-escapes | 4,107 | 2.98 | 3.76 | 19.04 | 6.38x |
| nested-64 | 135 | 1.62 | 1.68 | 1.27 | 0.78x |
| whitespace | 60,056 | 14.17 | 21.30 | 55.00 | 3.88x |

Walz takes longer than JSON.parse on most inputs: about 3.6–3.7x for record arrays, 3.3x for numeric arrays, and 10.6x for long ASCII strings.
It took about 1.07x for a 2,000-field object and 0.78x for 64 levels of nesting. Differences vary greatly with input shape and AST representation.
These measurements alone do not establish why individual functions are slow or the effect of a particular optimization.

<a id="jsから実際に呼ぶ場合"></a>

## Actual calls from JS

For Core＋JSPI / Component, an API that parses and returns only success status was measured separately from an API that parses and reserializes.
An API of the same type that returns only input length serves as a control, recording input-copy, UTF-8 conversion, and call costs. No values subtracting the control time are reported.

For a roughly 40 KB record array:

| Operation | Node | serde_json release | Core＋JSPI | Component |
|---|---:|---:|---:|---:|
| Parse + reserialize | 102.63 | 137.93 | 512.14 | 543.31 |
| Input ABI + parse, returning success status | — | — | 281.65 | 311.23 |
| Input ABI + return input length only | — | — | 74.17 | 97.15 |

Times for all conditions, raw samples, minima/maxima, MiB/s, and output byte counts are saved in the [fixed results](../benchmarks/json-parser/20261106-r2/report.json); readable tables are in [report.md](../benchmarks/json-parser/20261106-r2/report.md).
The status-only API does not return the AST, so it performs different work from the first table, which returns an AST.

<a id="条件と検査"></a>

## Conditions and checks

- Capture original text, Wasm, JS glue, native reference binaries, sources, and inputs before measurement, then verify SHA-256 hashes before running. Do not overwrite past captures or reports.
- Exclude compilation, instantiation, file I/O, preparation to place original text in GC, and result verification. Measurement processes do not load build tools.
- Adjust iterations per implementation to at least roughly 30 ms and warm up three times with the selected batch. Explicit GC before each measurement batch is outside the timed interval; automatic GC within the batch is included.
- Walz's parser body returns the actual GC AST to JS. Full reserialization and FNV-1a checks outside the timed interval prevent both elimination of unused results and measurement of incorrect results.
- Node creates JS objects returned by JSON.parse, Rust creates owned serde_json::Value, and Walz creates GC variants and lists. Rust also destroys each iteration's AST inside the timed interval; memory management and data representations are not identical.
- Rust runs in a native process with a resident UTF-8 string. Child-process startup and input loading are excluded, and each process parses 32 times before measurement. JS FFI is excluded.
- After all Core/Component batches, the ABI allocator's outstanding allocation count was verified to be 0. The runtime was not modified for measurement.

Speed-comparison inputs are restricted to safe-range integers, finite decimals, and sorted, unique keys.
Walz preserves numeric lexemes and duplicate keys, whereas Node's and serde_json's default Value/Map representations differ.
An additional check demonstrating numeric-lexeme preservation re-emitted 9007199254740993 unchanged in Walz and as 9007199254740992 in Node. This input was not used for speed rankings.

For Unicode newlines, Walz emits `\u000a`, while Node and serde_json emit `\n`. Checks compare JSON values rather than requiring identical string representations.
Output in this case is 2,443 bytes for Walz and 1,931 bytes for Node/serde_json; this output-size difference is also included in reserialization time.
Comparisons stay within Walz's 65,536-byte input and 128-level nesting limits; they do not compare invalid inputs, startup time, streaming, or browser performance.

<a id="再実行"></a>

## Rerunning

```sh
just test-json-bench
just json-bench-capture target/json-parser-bench/new-run
just json-bench target/json-parser-bench/new-run target/json-parser-bench/new-results 9
```

Artifacts for this run are in `target/json-parser-bench/20261106-r2/`, and results in `target/json-parser-bench/20261106-r2-results/`.
To remeasure the same capture, omit the second command and specify a new results directory.
Results, capture metadata, inputs, and runners are stored in VCS at [benchmarks/json-parser/20261106-r2](../benchmarks/json-parser/20261106-r2). Executable binaries remain under the local target directory.
