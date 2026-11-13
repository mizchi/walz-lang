# Direct row output and typed input staging profile

Captured artifact: `/Users/mz/ghq/github.com/mizchi/wlt-lang/benchmarks/table/io/20261113-r1/candidate-stream`. SHA-256: `7107613f87df71223055207c024f360d2c1988fbb3f60d4aa55c3705029fbf5a`.

Eight unchanged SIMD workloads, inspector CPU samples at 500µs, sampled JS allocations,
and exact export counts from a separate untimed instrumented instance. The sequential
timing pass is diagnostic; the rotating-order paired I/O suite is the speed comparison.
Profiled throughput is not speed evidence. Wasm GC allocations may be absent from
the JS sampling profiler. These estimates are not total allocation or leak measurements.

| Workload | Diagnostic ms | Export calls | Sampled JS MB/call | GC CPU % |
| --- | ---: | ---: | ---: | ---: |
| simd.compound | 1.623 | 38 | 0.36 | 0.3 |
| simd.simple | 1.151 | 22 | 1.23 | 2.4 |
| simd.ingest-only | 7.744 | 72 | 16.28 | 3.6 |
| simd.cold-ingest-filter | 9.162 | 93 | 20.11 | 3.3 |
| simd.materialize-only | 4.362 | 28 | 8.65 | 11.4 |
| simd.filter-materialize | 5.642 | 49 | 10.01 | 9.0 |
| simd.small-compound | 0.038 | 38 | 0.02 | 0.1 |
| simd.nullable-compound | 1.756 | 38 | 0.37 | 0.3 |

Read `profile.json` for complete samples, per-function CPU attribution, allocation
sites and export counts. `.cpuprofile` files can be opened in browser DevTools.
`analysis.json` preserves the original unfused two-scan counterfactual block model;
its work counts are not executed counts for the current fused one-scan artifact.

Replay into a new directory:

```sh
just profile-table benchmarks/table/io/20261113-r1/candidate-stream target/new-20261113-r4 simd.
```

See [the complete I/O suite](../../io/20261113-r1/README.md) for both paired matrices.
The primary-storage iteration substantially reduces sampled ingestion GC, but input
validation, numeric transfer, immutable list publication, and output construction
remain real costs. Numeric storage and SIMD caches can coexist; scalar/native full
views can additionally retain canonical Cells.
