# JSON parser speed comparison

v24.21.0 / Apple M5 / arm64; 9 rounds; median µs/call.

All logical JSON outputs and raw AST serialization hashes passed. AST/output validation, file I/O, compilation, instantiation and input setup are excluded. Calls rotate order each round. Explicit GC runs before each timed batch; automatic GC during a batch is included. Rust release Value destruction is inside its timer; each native process warms its allocator before timing.

WLT resident returns an opaque GC AST and is checked by a serialization hash outside the timer. Core/Component parse-status fully parses but returns only success, so it is a different API from returning an AST. Input-length measures the same input ABI without parsing; timings are not subtracted. Normalize includes both WLT ABI copies. Native Rust has no JS FFI or UTF-16 conversion. WLT preserves numeric lexemes and duplicates, JS uses Numbers, serde_json uses its default Value/Map. Ranked inputs use finite safe numbers and unique, sorted keys. Unicode escaping can differ (WLT uses \u000a for newline, Node uses \n); output sizes are recorded. Results apply to these inputs and representations.

## 既存の入力からASTを生成（ABIコピーなし）

| input | bytes | nodeParse | serdeParse | wltResident |
|---|---:|---:|---:|---:|
| small-object | 56 | 0.16 | 0.19 | 0.37 |
| records-128 | 9633 | 14.81 | 26.20 | 52.74 |
| records-512 | 40161 | 58.91 | 106.86 | 217.48 |
| numbers-4096 | 25747 | 40.47 | 47.70 | 132.36 |
| wide-object | 24891 | 121.22 | 181.06 | 129.89 |
| ascii-string | 48002 | 12.09 | 3.17 | 128.74 |
| unicode-escapes | 4107 | 2.98 | 3.76 | 19.04 |
| nested-64 | 135 | 1.62 | 1.68 | 1.27 |
| whitespace | 60056 | 14.17 | 21.30 | 55.00 |

## JS API：入力変換を含む解析と入力長だけの対照

| input | bytes | coreInputLength | coreParseStatus | componentInputLength | componentParseStatus |
|---|---:|---:|---:|---:|---:|
| small-object | 56 | 1.02 | 1.29 | 3.20 | 3.84 |
| records-128 | 9633 | 18.50 | 69.13 | 26.31 | 78.55 |
| records-512 | 40161 | 74.17 | 281.65 | 97.15 | 311.23 |
| numbers-4096 | 25747 | 17.85 | 125.23 | 43.70 | 154.69 |
| wide-object | 24891 | 17.33 | 135.49 | 40.93 | 159.02 |
| ascii-string | 48002 | 32.19 | 152.37 | 80.88 | 214.24 |
| unicode-escapes | 4107 | 3.43 | 20.94 | 10.14 | 28.68 |
| nested-64 | 135 | 1.09 | 2.12 | 3.69 | 5.05 |
| whitespace | 60056 | 40.71 | 95.98 | 99.60 | 159.14 |

## 解析＋再シリアライズ（WLTは入出力ABI込み）

| input | bytes | nodeNormalize | serdeNormalize | coreNormalize | componentNormalize |
|---|---:|---:|---:|---:|---:|
| small-object | 56 | 0.26 | 0.25 | 1.88 | 4.26 |
| records-128 | 9633 | 25.35 | 34.06 | 118.52 | 129.08 |
| records-512 | 40161 | 102.63 | 137.93 | 512.14 | 543.31 |
| numbers-4096 | 25747 | 98.07 | 77.55 | 199.06 | 223.70 |
| wide-object | 24891 | 197.53 | 202.03 | 236.94 | 264.58 |
| ascii-string | 48002 | 25.42 | 15.04 | 340.76 | 396.34 |
| unicode-escapes | 4107 | 3.90 | 4.80 | 32.78 | 41.47 |
| nested-64 | 135 | 3.31 | 2.13 | 4.01 | 6.59 |
| whitespace | 60056 | 14.27 | 21.32 | 96.71 | 159.69 |
