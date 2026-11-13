# Table filter profile and remaining optimizations

The main opportunities are bulk input/output, avoiding duplicate row scans,
and reducing work in numeric predicate plans. Increasing SIMD use alone does
not address the current JS-facing costs.

This capture executes the unchanged [suite-1 final Wasm and facade](../../20261113-r2/)
on Apple M5 / Node.js 24.22.2. It contains twelve workloads: scalar/packed/SIMD
compound filters, simple filtering, ingestion, cold ingestion plus query,
materialization, query plus materialization, two Arquero controls, a 4,096-row
case and a nullable case. The main input has 262,144 rows and three columns.

- [CPU, sampled allocation, call counts and separate timings](profile.json)
- [Reproducible derived statistics and block model](analysis.json)
- Raw `*.cpuprofile` / `*.heapprofile` files alongside this report
- [Initial exploratory block calculation](derived.json); `analysis.json` is the
  maintained, tested calculation and reproduces its comparison counts

## What was observed

Unprofiled diagnostic medians, with at least 500 ms / 32 calls warmup:

| Operation | Time |
| --- | ---: |
| SIMD resident simple filter | 2.929 ms |
| SIMD resident compound filter | 4.550 ms |
| Ingestion only | 32.801 ms |
| Fresh ingestion + simple filter | 37.561 ms |
| Materialization only, 136,793 selected rows | 49.453 ms |
| Simple filter + materialization | 51.635 ms |
| Arquero filter + materialization | 5.041 ms |

These cases run sequentially, not as a rotating paired comparison. Their longer
warmup and shared V8 module instances differ from the original benchmark matrix.
In particular, this capture's SIMD/packed-scalar ranking differs from suite 1;
do not use it as a new general SIMD speedup claim. CPU/heap profiling runs in
separate passes after all timings. Phase times are separate measurements, not
an exact additive decomposition of one call.

CPU self-sample observations:

| Workload | Hot area | Raw self share |
| --- | --- | ---: |
| SIMD compound | `numeric-mask`, including inlined comparisons | 68.5% |
| SIMD compound | `filter-packed` | 24.9% |
| SIMD simple | `filter-packed`, including its inlined leaf | 82.5% |
| SIMD simple | `prefix` and `__list_from` combined | 9.6% |
| Ingestion | JS-to-Wasm export wrappers | 48.6% |
| Ingestion | GC | 24.6% |
| Materialization | JS-to-Wasm export wrappers/getters | 36.3% |
| Materialization | repeated column-name lookup (`#index`) | 6.8% |

These raw percentages retain roughly 4.6–5.9% inspector transition overhead.
Self percentages are useful for ranking; inclusive percentages overlap and
must not be added. Inlining and stack sampling prevent assigning the entire
`numeric-mask` share solely to tree traversal or solely to vector arithmetic.
`walz-file-2` is the table engine and `walz-file-1` is the number-column helper.

## Priorities

### 1. Transfer columns in batches and materialize without the scalar getter path

The untimed export counter records **1,572,900 calls** for ingestion and
**1,641,517 calls** for materialization. Ingestion calls `cell-new` and `cell-set`
786,432 times each. Materialization calls `get`, `cell-kind` and `cell-number`
410,379 times each, plus 410,380 `num-rows` calls.

[`table()`](../../../../runtime/table/index.mjs) constructs every cell across the
boundary. `objects()` calls public `get(name,row)` for every value, repeating
name lookup and row-bound checks, then creates entry arrays with `map` and
`Object.fromEntries`. Sampled visible heap allocation for materialization is
about 57.8 MB/call, versus 10.9 MB/call for Arquero's filter plus materialization.
Allocation sampling is approximate, may expose only part of Wasm GC allocation,
and is not a total-allocation or leak guarantee.

Add typed column batches with explicit kind/null/undefined masks, preserving
NaN and signed zero. Numeric data should cross in a small number of bulk calls.
Keep the existing generic scalar path for unsupported column types. Reuse the
existing ABI buffer ownership/cleanup contracts rather than adding a persistent
handle registry. A smaller first step is a private materialization reader that
hoists column indices and row count; it still leaves most per-cell calls.
Preserve plain-object output and special column names such as `__proto__`.

### 2. Scan once when base membership and logical order are known identical

[`filter-mode`](../../../../walz/table/engine.walz) always filters `$rows` and
`$order` independently. Fresh tables use the same identity selection for both.
Track this invariant explicitly and reuse the selected list when it holds.
Retain separate scans for differently ordered tables, including sorted/sliced
grouping cases. This removes one scan and its allocations in the common case;
it is not evidence of a universal 2x end-to-end speedup.

### 3. Short-circuit whole SIMD pair masks

[`numeric-mask`](../../../../walz/table/engine.walz) eagerly evaluates both sides
of every AND/OR. Its numeric planner has already restricted leaves to immutable,
total operations. Therefore AND may skip the right subtree when the left mask
is zero, and OR may skip it when both bits are one. Other expression trees must
retain the scalar fallback and its original errors/short-circuit behavior.

On the captured compound input, **83,863 / 131,072 pairs (63.98%)** have no rows
in the `x` range. They need no `y` comparisons. Including lower-bound and OR
shortcuts, the tested source-level model reduces comparison leaves from
**1,048,576 to 648,054 (38.20%)** across the two current scans, with identical
result masks. This is a work-count prediction, not measured optimized throughput.

### 4. Compile common predicate shapes into fused block loops

The current loop traverses a recursive plan and dispatches operators per two
rows. Each comparison returns an i32 mask before Boolean composition; repeated
column range checks reload the same column through separate plan leaves.
Specialize common single/range/Boolean shapes once before scanning, reuse block
loads and splatted constants, and keep vector masks through AND/OR until a final
bitmask conversion. Separate scalar/vector loop dispatch rather than carrying
the mode flag through every recursive call. Keep a checked generic fallback.
The profile identifies the hot region but does not prove which individual
instruction transformation will win; measure each specialization against the
same packed-scalar control and consider selective block short-circuiting too.

### 5. Remove redundant selection-array copies with an explicit ownership invariant

For each pass, `filter-packed` allocates capacity for all input rows. `prefix`
then allocates/copies the selected prefix, and
[`@list.from-array` emission](../../../../src/lists.rs) copies it again to publish
an immutable list. Across two passes this is six backing arrays.
For the simple filter, the raw array payload is `8*n + 16*m = 4,285,840` bytes
(`n=262144`, `m=136793`), close to the approximately 4.30 MB visible allocation
estimate. The copy helpers occupy about 9.6% of raw self samples in that case.

A private immutable selection view with logical length, or a consuming builder
finish, could avoid copies. Do not change general `from-array` snapshot semantics
or expose mutable backing storage. Retaining excess capacity is a memory tradeoff.
Count-only fusion would avoid row IDs but is a different result contract from
returning a reusable filtered table.

### 6. Keep scalar filter predicates unboxed where possible

The scalar compound profile spends 46.3% of raw self samples in `binary` and
25.8% in `eval`. Its sampled visible allocation estimate is about 138.8 MB/call,
versus about 2.52 MB/call for the packed plan. A typed predicate evaluator can
return Boolean/numeric temporaries without allocating general cells, particularly
for currently unsupported fast-path expressions. Preserve string conversions,
executed-branch traps and short-circuit behavior. Do not globally intern arbitrary
GC values or change observable reference identity as a compiler shortcut.

## Existing optimizer not yet used by the table build

The regular table builder invokes `walz build` without Binaryen postprocessing.
A [separate `wasm-opt` 133 probe](../../probes/20261113-wasm-opt-r2/probe.json)
applies `-O3 -g` with GC/SIMD features, retaining the export surface. It uses no
closed-world or fast-math assumptions. Same input/facade, nine rotating samples:

| Resident filter | Original | `-O3` | Ratio |
| --- | ---: | ---: | ---: |
| SIMD simple | 3.138 ms | 3.013 ms | 1.04x |
| SIMD compound | 4.340 ms | 4.021 ms | 1.08x |
| Packed scalar compound | 7.398 ms | 6.035 ms | 1.23x |

Wasm size with names retained falls from **38,084 to 20,449 bytes**. This is a
useful optional build step, but the measured effect does not remove the larger
input/output and plan-execution costs. The probe checks complete edge-case rows,
sort/slice behavior and traps on the raw Core facade; it is not a complete
Core + JSPI / Component or browser release validation. The default build is
unchanged. The failed first probe is retained with its test-oracle error in
[`20261113-wasm-opt-r1`](../../probes/20261113-wasm-opt-r1/).

## Lower priority

Acorn's self share is about 0.5% for the large SIMD compound case and 3.6% for
the 4,096-row case. Expression/schema caching can help repeated small queries,
but it is behind scan/plan work and bulk adapters for these workloads. Any cache
must respect schema/instance identity and avoid retaining discarded tables.
Turning off bounds checks or relaxing IEEE/GC semantics is unnecessary.

## Reproduce

Run from the repository root; every output directory must be new:

```sh
just test-table-profile
just profile-table benchmarks/table/20261113-r2 target/table-profile/new-run
just probe-table-wasm-opt benchmarks/table/20261113-r2 target/table-profile/new-opt
```

Profiling validates hashes and executes the captured Wasm/facade, without
rebuilding current language sources. CPU and sampled-heap passes are separate;
export counting uses a separate untimed wrapper instance. No profilers/builds/
tests run alongside timing samples. The additional analysis tool and its tested
helper are captured under `analysis-tool/`; the original capture tools remain
unchanged beside the raw profiles. Whole-result differential checks run outside
clocks. New runtime optimizations should get fresh differential/cleanup/backend
checks and new paired benchmark captures before adoption.
