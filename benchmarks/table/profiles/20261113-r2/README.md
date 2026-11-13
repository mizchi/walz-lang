# Profile after the five tuning stages

This capture profiles the frozen [final tuning artifact](../../tuning/20261113-r1/final/),
using the same twelve workloads as the [before profile](../20261113-r1/README.md).
Actual speed comparisons belong to the separate rotating-order
[tuning-v2 matrix](../../tuning/20261113-r1/README.md); these diagnostic timings
and profile samples must not be mixed into that matrix.

## Export calls and allocation estimates

| Operation | Before export calls | After export calls | Before sampled bytes/call | After sampled bytes/call |
| --- | ---: | ---: | ---: | ---: |
| Ingest 262,144 rows × 3 columns | 1,572,900 | 72 | 45.98 MB | 52.61 MB |
| Materialize 136,793 rows × 3 columns | 1,641,517 | 28 | 57.81 MB | 15.36 MB |
| Simple filter + materialization | 1,641,538 | 49 | 62.77 MB | 16.59 MB |
| Resident simple filter | 22 | 22 | 4.30 MB | 1.23 MB |
| Resident compound filter | 38 | 38 | 2.52 MB | 0.36 MB |

Counts are exact function-export counts in a separate untimed wrapper instance;
they exclude memory-buffer property reads and native calls inside Wasm.
Input/output now scales with columns and 65,536-row chunks instead of cells.
No `cell-set` exports are called for numeric ingestion, and no per-cell `get`,
`cell-kind` or `cell-number` exports are called during numeric materialization.

Sampled allocation is approximate and is not a total-allocation or leak proof.
Ingestion allocates temporary JS chunk snapshots, so its sampled bytes/call
increase even though its time and export-call count improve. These snapshots
are temporary; the retained transfer memory stays bounded at 640 KiB. The table
continues to own GC cells and immutable column snapshots. Reducing Cell allocation
requires a different primary numeric representation and remains follow-up work.

## Bottlenecks shifted

- Ingestion: GC is about **55.2%** of raw CPU self samples, JS table construction
  19.6%, native `cells-import` 10.0%. Per-cell JS-to-Wasm wrappers are no longer
  dominant. More SIMD in the filter does not remove these ingestion costs.
- Materialization: `objects()` accounts for about **74.8%** of raw self samples;
  native `column-export` is about 3.6%. Output row-object construction is now the
  main cost. Columnar consumers should avoid creating row objects when practical.
- Compound filtering: `filter-packed` is about 48.1%, fused comparison pairs
  19.1%, `numeric-mask` 15.0%, and mask collection 10.5%. This profile includes
  inlining; it does not isolate individual comparison instructions.
- Simple filtering: mask collection is about **55.0%**, scan/comparison about
  31.2%, and final immutable list copying about 5.3%. Further query work should
  examine selection compaction, dense/sparse cases, and count-only result sinks.

Raw CPU fractions include about 5% inspector transition overhead. Profiling runs
after all unprofiled timing samples, and allocation sampling runs separately from
CPU sampling. The scalar control still allocates generic expression cells;
unboxed scalar predicates remain another possible improvement.

## Files and replay

- [profile.json](profile.json): full CPU summaries, sampled allocations,
  diagnostic timing samples and exact export-call counts
- [analysis-final.json](analysis-final.json): derived summaries with corrected
  scope wording for the old block model
- Original `analysis.json` is retained. Its block counts are the original
  **unfused two-scan counterfactual**, not executed call counts for this final
  one-scan/fused artifact. The modeled comparison counts are unchanged; only the
  later analysis clarifies their scope.
- Raw `*.cpuprofile` / `*.heapprofile` files and capture-time tools are preserved.
  The later derived-analysis source is in `analysis-tool/`.

```sh
just test-table-profile
just profile-table benchmarks/table/tuning/20261113-r1/final target/table-profile/new-after
```

The artifact is labeled `walz-table-artifact-v2`; it is not a historical suite-1
measurement. Hash validation accepts this artifact format and keeps the original
suite-1 validation path. The replay does not rebuild current language sources.
