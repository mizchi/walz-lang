# Profile-guided table tuning

Five stages improve resident numeric filters and numeric I/O without adding
language syntax or weakening table/list snapshot semantics. The captured code
uses checked GC storage, scalar tags, schema validation and existing effects.
The [after profile](../../profiles/20261113-r2/README.md) confirms the drop in
per-cell export calls and selection allocation, and records remaining costs.

## Results

Apple M5 / Node.js 24.22.2, 262,144 numeric rows, milliseconds per operation:

| Operation | Original | Final | Arquero 8.0.3 |
| --- | ---: | ---: | ---: |
| Resident simple filter | 3.030 | 1.252 | 2.781 |
| Resident compound filter | 5.218 | 1.898 | 3.214 |
| Fresh ingestion + simple filter | 38.894 | 11.776 | 3.117 |
| Simple filter + JS objects | 55.731 | 20.050 | 6.179 |

Resident filters improve by about **2.42x / 2.75x** over the original and
**2.22x / 1.69x** over Arquero here. Cold and materialized operations improve by
about **3.30x / 2.78x**, but remain slower than Arquero. This does not establish
an end-to-end advantage for replacing a complete JS table library.

With null/undefined/NaN in the input, resident simple/compound filters take
**1.367 / 2.014 ms**, versus the original's **3.216 / 5.629 ms** and Arquero's
**3.424 / 4.197 ms**. At 4,096 numeric rows, the compound query is approximately
tied with Arquero (36.10 versus 36.24 us). Results depend on size and workload.

## Stages

All stage sources and executed Wasm files are frozen beside this report.
For 262,144 numeric rows, milliseconds per resident operation:

| Stage | Simple | Compound |
| --- | ---: | ---: |
| Original | 3.030 | 5.218 |
| 1: bounded bulk numeric I/O | 2.911 | 5.557 |
| 2: reuse aligned base/order selections | 1.860 | 3.012 |
| 3: pair-mask short-circuiting | 1.820 | 2.617 |
| 4: fuse same-column ordered comparisons | 1.811 | 2.278 |
| 5: masks then exact row-ID allocation | 1.252 | 1.898 |

Stage 1 mainly changes ingestion/materialization; it is not a resident query
optimization. Stage 5 also includes small JS validation cleanup, big-endian
fallback and moving unused aligned-slice allocations behind their branch.
Do not attribute ingestion-only differences in stage 5 to row-mask allocation.

Final packed-scalar controls take 1.580 / 3.366 ms for the two numeric filters.
Scalar and SIMD share the same packed numeric cache, aligned scan invariant and
row-mask collection, but only SIMD uses fused vector comparison pairs.

### Bulk I/O

The raw Core facade batches numeric/Boolean/nullish values in at most 65,536-row
chunks. One memory has a 16-byte private header and at most ten 64-KiB pages.
It starts with one page, grows on demand and retains at most **640 KiB** per
instance. Data is encoded as one kind byte per row followed by aligned f64
values; published columns copy values to GC storage. Borrow state closes in
`finally`, including traps. Results are JS arrays/objects, not borrowed views.
No handle registry or per-table manual disposal was added.

Strings keep the generic bridge; big-endian hosts use scalar I/O. The `io:
"scalar"` option retains a per-cell control. `objects()` hoists column indices
and row count, and reuses temporary entry pairs before `Object.fromEntries`,
preserving ordinary object prototypes and own `__proto__` properties.

### Scan and predicate work

An immutable internal flag records when base membership and logical order are
identical. Constructors, projections, grouping and filtering preserve it;
sorting clears it, and unorder restores it. Filtering scans once when it holds.
Sorted selections keep separate scans and retain original group-discovery order.

Only the numeric planner's total, immutable leaves can use pair-mask shortcuts.
AND skips a zero left mask, OR skips a full left mask. General expressions still
use the scalar evaluator and preserve executed-branch traps. Same-column ordered
comparison pairs combine vector masks before one bitmask conversion, reusing a
single load/gather. Complex trees keep the generic plan around these fused pairs.
This is not whole-plan JIT compilation or arbitrary expression fusion.

### Selection storage

Numeric filtering first stores a byte per two logical input rows, counts selected
bits, allocates exact row-ID capacity, then uses ordinary `@list.from-array` to
publish an immutable snapshot. The previous extra prefix allocation/copy is
removed. General list constructor copy behavior is unchanged.

For this simple filter (`n=262144`, `m=136793`), row-array payload changes from
**4,285,840 bytes** in the original two-scan path to **1,225,416 bytes** for the
final aligned scan (`ceil(n/2) + 8*m`), excluding GC headers. This is an allocation
model, not a leak proof. Source columns and numeric caches may remain retained
by a small selection as documented in the table guide.

## Method and provenance

[results.json](results.json) contains all 15 comparison groups, nine samples per
variant, warmup counts and iteration counts. It compares the original capture,
five stages, official Arquero and the final packed-scalar control, rotating order
each round. The inputs are copied byte-for-byte from the historical suite-1
capture; hashes are checked before and after execution. The new **tuning-v2**
suite uses at least 500 ms / 32 calls warmup and 20-ms target batches, so its
samples must not be mixed with the older suite's timings.

Complete output rows are checked against independent JS filtering outside
clocks, and every timed result contributes a count checksum. Explicit GC is
outside clocks. No builds, tests or profilers overlap timing samples. All builds
and backend/browser checks finish first. The phases are separate tasks, not an
additive decomposition of one call. Ingestion-only Arquero results are metadata
construction over borrowed host columns; Walz owns a snapshot, so these have
different ownership costs. The original and final Walz snapshots have the same
ownership behavior. No Binaryen postprocessing is applied in this comparison.

Compiler/core sources match the original 136-file inventory. The native CLI
hash recorded in `provenance.json` immediately after `cargo test` differs from
the Cargo-run build. The [recheck](compiler-recheck.json) rebuilds the original
source with `cargo run`: both its compiler hash and emitted Wasm bytes match
the historical baseline exactly. All executed stage inventories and the final
source-to-artifact correspondence are also checked.

`stage1/` through `stage5/` hold the frozen stage inputs/outputs. `final/` is an
explicitly labeled **artifact-only v2** capture for the existing profiling
workloads; it is not a fabricated suite-1 benchmark result.

```sh
# Replay the comparison using the saved stages; choose a fresh output directory.
just bench-table-tuning benchmarks/table/20261113-r2 benchmarks/table/tuning/20261113-r1 target/table-tuning/new-run
just profile-table benchmarks/table/tuning/20261113-r1/final target/table-tuning/new-profile
```

Functional validation covers native Wasmtime, checked IR/effects/opaque storage,
all comparator modes, bool/null/undefined/NaN/signed-zero cases, vector tails,
sorted/sliced/grouped pipelines, joins, GC reachability, boundary call counts,
borrow failure/recovery, bounded memory reuse, TypeScript, both Core + JSPI and
Component value adapters, and Chromium. The later profile is separate from all
unprofiled samples.
