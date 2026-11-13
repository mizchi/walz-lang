# Table input/output tuning

This run measures existing JS column arrays → Walz ingestion → a filter →
ordinary JS row objects in one clock. It includes input snapshots/copies,
expression construction, first SIMD packing, selection, row construction and
runtime GC. Module loading, instantiation, network I/O and JSON serialization
are outside the clock. These are actual facade costs: Walz snapshots the inputs,
whereas Arquero can retain their JS columns without copying them.

The official reference is pinned Arquero 8.0.3 on Node.js 24.22.2, Apple M5,
darwin arm64. Six captured inputs cover 4,096 / 65,536 / 262,144 rows, numeric
and nullable, with the same three columns. Every variant receives the same input
object. Nine sample rounds rotate order, following at least 500ms/32 calls of
warmup. Explicit GC, full-row checks and count checks are outside clocks; GC
triggered within a call remains part of that call. Builds, tests and profiles
are stopped during timing. Raw samples, batches and hashes are retained.

`before` is the frozen five-stage artifact in
`../../tuning/20261113-r1/final/`, measured again in this suite. The candidates:

- `candidate/`: initial primary-storage capture, with per-path membership unions
  in the compiler's region analysis; no timed benchmark uses this capture.
- `candidate-final/`: the first input/output iteration. Numeric/tag lists replace
  eager Cells, with canonical lazy Cell views. Row construction uses a property
  template, after extracting JS column arrays.
- `candidate-stream/`: the same Wasm bytes, with local typed input staging and
  direct transfer into result rows. It removes boxed staging and intermediate
  result columns. Getters run once before borrowing Wasm scratch, including
  reentrant getters. Strings retain the generic Cell fallback.

The checked Wasm in all candidates is byte-identical. The final compiler
implementation uses dependency-driven synchronous region rounds, memoized
substitution and exact sorted-set unions. Differential tests compare every
summary fact with the original schedule/transfer, including unknown roots and
recursive caches; no checks or facts are skipped. Complex native table analysis
still takes minutes and is tracked in TODO.md.

`paired/` records the first iteration; `paired-stream/` records the final one.
Compare before/after/reference within each matrix. Do not combine their samples
or treat these as the older filter-only suite. Native accessor cache effects
are documented in [the table guide](../../../../docs/table.md).

Replay the final artifacts without rebuilding:

```sh
just bench-table-io benchmarks/table/tuning/20261113-r1/final \
  benchmarks/table/io/20261113-r1/candidate-stream target/new-table-io-run
```

The output directory must not exist. Input, executable Wasm and facade hashes
are checked before and after the run. The executed harness is copied into each
result directory; its installed Arquero version is checked explicitly.

## Final paired results

Milliseconds per call, medians of nine samples. The complete cold pipeline is
`table(data).filter(predicate).objects()`. No cross-run samples are combined.

| Input | Rows | Predicate | Before | After | Arquero |
| --- | ---: | --- | ---: | ---: | ---: |
| numeric | 4096 | simple | 0.413 | 0.200 | 0.077 |
| numeric | 4096 | compound | 0.187 | 0.181 | 0.062 |
| nullable | 4096 | simple | 0.390 | 0.211 | 0.075 |
| nullable | 4096 | compound | 0.215 | 0.179 | 0.066 |
| numeric | 65536 | simple | 6.580 | 3.449 | 1.049 |
| numeric | 65536 | compound | 3.616 | 2.947 | 0.781 |
| nullable | 65536 | simple | 6.582 | 3.254 | 1.090 |
| nullable | 65536 | compound | 3.543 | 2.721 | 0.875 |
| numeric | 262144 | simple | 39.517 | 15.347 | 7.006 |
| numeric | 262144 | compound | 14.884 | 11.683 | 3.228 |
| nullable | 262144 | simple | 25.803 | 13.898 | 4.697 |
| nullable | 262144 | compound | 15.409 | 11.795 | 3.887 |

The final artifact improves these complete pipelines by 1.03–2.57x in this run,
but remains slower than Arquero. Arquero can wrap host columns without taking
Walz’s immutable input snapshot. Small-input ingestion does not consistently
improve; do not treat the new representation as a universal speedup.

At 262,144 numeric rows, isolated phases are:

| Phase | Before | After | Arquero |
| --- | ---: | ---: | ---: |
| ingest-only | 7.984 | 7.438 | 0.000111 |
| resident-compound | 1.610 | 1.627 | 2.647579 |
| materialize-only | 16.591 | 4.110 | 1.838254 |
| filter-materialize | 17.986 | 5.645 | 4.509417 |

The numeric complete-simple case is noisy: individual samples span 29.92–78.03ms
(before), 14.40–18.78ms (after), and 5.13–10.63ms (Arquero). All samples are
retained, including the slow ones. The resident filter/output phase is much
more stable. The observed cold median ratio is not a guaranteed speedup.

## Profiles and remaining work

The [first iteration profile](../../profiles/20261113-r3/README.md) and
[final profile](../../profiles/20261113-r4/README.md) are separate diagnostic
passes, not additional paired speed evidence. The final input CPU profile puts
`readChunk` at 40.6%, `numeric-import` at 19.8%, constructor validation at 8.0%,
and list publication at 7.7%. Reducing object allocation alone does not remove
host validation and snapshot/copy costs.

Next: a checked opaque builder could validate during mutation and publish a
copied immutable store, avoiding repeated validation without an unchecked
constructor. Keep string, mixed wire-tag, scalar execution and browser fallbacks.
A complete columnar output API could avoid ordinary row construction entirely.
