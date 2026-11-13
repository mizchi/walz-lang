# Numeric table filter measurements

The [complete input/output suite](io/20261113-r1/README.md) compares the previous
five-stage artifact with primary numeric storage and template-based JS row
materialization. It measures fresh JS columns → filter → JS row objects in one
clock, alongside isolated phases. Its timings are independent of suite 1 below.

[Profile-guided tuning](tuning/20261113-r1/README.md) implements bulk numeric I/O,
one aligned scan, block-mask short-circuiting, comparison fusion and exact row-ID
allocation. Its separate tuning-v2 matrix preserves all five stages and the
original inputs; do not mix its samples with suite 1 below.

The [CPU/allocation profile and remaining optimization priorities](profiles/20261113-r1/README.md)
cover bulk adapters, duplicate scans, block-mask short-circuiting, fused plans,
selection copies, unboxed predicates, and a separate `wasm-opt` probe.

This suite measures the generic Arquero-style table engine, independently of
the specialized dictionary/SIMD collection suites. Packing numeric cells improves
on the original cell interpreter, but a SIMD instruction alone does not establish
an advantage over packed scalar comparisons or official Arquero.

## Method

Run from the repository root with Node.js 24+:

```sh
just bench-table benchmarks/table/new-run
```

The output directory must not exist. Suite version 1 uses fixed xorshift32 input
at 4,096, 65,536 and 262,144 rows, with three columns (`x`, `y`, `id`), both
numeric and nullable data. It times:

- Resident `x < 20`, returning a filtered table and consuming its row count.
- Resident `x >= -100 && x < 100 && (y < 32 || y >= 96)` with the same output.
- Fresh table ingestion, first `x < 20` query/cache preparation and row count.
- Resident `x < 20` plus full JS object materialization.

Controls are the original cell interpreter, packed scalar comparisons with the
same v128 cache and selection loop, f64x2 comparisons, and pinned official
Arquero 8.0.3. A direct JS count loop is a lower-bound control for resident count
tasks: it does not allocate a filtered table or row-ID selection. These controls
distinguish data layout/interpreter effects from vector comparison effects.

The Walz facade snapshots input cells into Wasm; Arquero can reuse host columns.
Cold timings include those real API/ownership costs. Resident timings exclude
ingestion and amortize numeric packing; materialization timings include the
actual per-cell GC bridge to JS. Sums are not vectorized or reassociated.

Each variant warms for at least 120 ms and eight calls. Batches target 20 ms
with variant-specific iteration counts. Nine rounds rotate variant order.
Explicit forced GC is outside clocks; any runtime GC during an operation is
part of its timing. Every call consumes a count, and full rows are checked
against independent JS filtering and official Arquero outside clocks.
Builds, tests and profiling are stopped before timing.

Each run saves tagged inputs, executable source snapshots, the broader source
hash inventory, compiler hash, executed Wasm, environment, iterations and every
sample in `results.json`. Archived runs are not overwritten or presented as
measurements of later code.

## Initial integration

[20261113-r1/results.json](20261113-r1/results.json) captures the initial lazy
cache/SIMD integration on Apple M5, Node.js 24.22.2, darwin arm64.
At 262,144 numeric rows, milliseconds per complete operation:

| Operation | Arquero | Original cells | Packed scalar | SIMD |
| --- | ---: | ---: | ---: | ---: |
| Resident simple filter | 2.634 | 8.026 | 3.078 | 3.186 |
| Resident compound filter | 3.013 | 20.081 | 5.668 | 6.783 |
| Fresh ingestion + filter | 3.133 | 53.742 | 43.166 | 39.056 |
| Filter + JS objects | 6.352 | 64.350 | 54.737 | 58.898 |

SIMD improves on the original interpreter by 2.52x / 2.96x for these resident
filters, but is slower than the packed scalar control. It does not beat Arquero
on these tasks. Cold and output costs remain much larger than Arquero's;
resident speedups should not be described as end-to-end replacement wins.

## After removing ordered-comparison mask reads

[20261113-r2/results.json](20261113-r2/results.json) captures the final code.
Only `number-column.walz` changes in the executable source snapshots; inputs,
tasks, controls, method and compiler hash match the first run. Equality still
reads type masks; ordered comparisons return their numeric mask directly.
At 262,144 numeric rows, milliseconds per complete operation:

| Operation | Arquero | Original cells | Packed scalar | SIMD |
| --- | ---: | ---: | ---: | ---: |
| Resident simple filter | 2.867 | 8.298 | 3.248 | 3.237 |
| Resident compound filter | 3.194 | 20.436 | 5.360 | 7.042 |
| Fresh ingestion + filter | 3.258 | 55.934 | 41.105 | 39.715 |
| Filter + JS objects | 6.041 | 60.565 | 54.053 | 53.711 |

The final SIMD path is 2.56x / 2.90x faster than the original interpreter on
the two resident filters. Simple comparisons tie packed scalar, and compound
SIMD comparisons remain slower. The small code change does not establish a
general timing improvement over the first run. Nullable simple filtering is
3.253 ms versus Arquero's 3.356 ms, a small machine/workload-specific margin;
this is not a general advantage over Arquero. Ingestion and materialization
remain substantially slower than the reference.

Further tuning should profile plan traversal/dispatch, row-selection allocation
and duplicate base/order scans, then measure specialized block plans and bulk
column adapters. Count-only fusion is a separate task from returning a filtered
table. Keep packed scalar as a control and do not attribute layout gains solely
to SIMD. This suite does not time joins, sorting or aggregation and does not
establish a replacement win for a complete JS table library.

[Validation logs](validation/) include the pre-implementation failing SIMD IR
assertion, native execution, official-reference differential checks, Core + JSPI
and Component cleanup, TypeScript checks, and Chromium checks.
