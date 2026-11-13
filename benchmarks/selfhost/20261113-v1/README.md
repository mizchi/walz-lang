# Initial selfhost / full Rust compiler comparison (v1)

Recorded on 2026-11-13T10:45:09.499Z; Apple M5, v24.22.2, rustc 1.99.0 (b940084d7 2026-09-28).

All eight inputs pass exact canonical-module equality for Core and Component.
Execution probes include wrapping integers, recursion, loops and traps.

| Input | Source bytes | Rust full API µs | Core µs | Component µs | Rust/Core |
| --- | ---: | ---: | ---: | ---: | ---: |
| integer-mix | 273 | 35.18 | 4.55 | 9.18 | 7.74× |
| i64-loop | 361 | 27.80 | 4.84 | 9.67 | 5.74× |
| recursive-call | 244 | 36.68 | 3.45 | 7.85 | 10.62× |
| branches-and-traps | 317 | 42.02 | 4.25 | 8.62 | 9.89× |
| i64-bits | 346 | 44.44 | 5.79 | 10.60 | 7.68× |
| wide-indices | 9630 | 1608.49 | 181.13 | 195.88 | 8.88× |
| expressions-32 | 9104 | 1291.69 | 129.34 | 140.35 | 9.99× |
| expressions-128 | 36081 | 4980.74 | 573.57 | 615.97 | 8.68× |

These are medians of nine sample means for resident compilation. Loading, startup, file/JSON I/O and verification are excluded. Every measured compile output is checked against its captured bytes.

The Rust compiler performs additional structured analysis. This v1 compares public compiler APIs, not equal internal passes. v2 adds a Rust WAT-parser-plus-validator reference to expose that difference; v1 data is preserved unchanged.

The i64-loop check uses the Rust native-WAT fallback. Check timings use walz::validate; compile timings use walz::compile.

Raw Wasm bytes differ because of debug names/type interning/local grouping. Exact canonical bytes preserve signatures, function/local/export order and every instruction, and are equal for both guests and Rust. This is an artifact-level contract for a closed scalar subset, not a proof for arbitrary Walz source.

Raw samples and methodology: [results.json](results.json). Frozen inputs, probes and outputs: [cases](cases/). Source/tool hashes: [capture.json](capture.json). The complete local runtime/native/source snapshot is retained at /Users/mz/ghq/github.com/mizchi/wlt-lang/target/selfhost-bench/capture-v1-r1. The code copies here are provenance records; replay uses the frozen source/runtime/selfhost-bench.mjs in that complete capture.
