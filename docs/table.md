# Table processing with Walz

`walz/table/` implements column-oriented table transformations in Walz. The JS
facade uses an [Arquero](https://github.com/uwdata/arquero)-style fluent API.
The first version targets the main relational verbs and scalar data; it is not
a drop-in replacement for every Arquero expression, type, or I/O format.
Differential tests use the pinned official Arquero 8.0.3 development dependency.

The implementation is independent code under this repository's Apache-2.0
license. Arquero is the API/behavior reference, distributed under BSD-3-Clause.
It is not imported by the runtime facade. Acorn parses expression syntax; it
does not execute user functions or compute table rows.

## Build and use

```sh
just build-table
node examples/table/demo.mjs
```

```js
import { instantiate, op, desc } from "./runtime/table/index.mjs";

const aq = await instantiate(new URL("./target/table/", import.meta.url));
const sales = aq.table({
  country: ["JP", "US", "JP", "FR"],
  amount: [20, 100, 22, 3],
  units: [2, 1, 3, 4]
});

const totals = sales
  .filter(d => d.amount >= 10)
  .derive({ revenue: d => d.amount * d.units })
  .groupby("country")
  .rollup({ count: op.count(), revenue: op.sum("revenue") })
  .orderby(desc("revenue"))
  .objects();
// [{ country: "JP", count: 2, revenue: 106 },
//  { country: "US", count: 1, revenue: 100 }]
```

Pass a directory URL ending with `/`. The builder produces `core.wasm`; normal
ESM bundlers resolve the facade's Acorn dependency for browser applications.
The facade requires Wasm GC support. Node.js 24+ is used for tests.

## Public API

| API | Behavior |
| --- | --- |
| `table({name: values})`, `from(objectRows)` | Snapshot input scalar values; columns must have equal lengths |
| `select(...names)` | Project/reorder columns; retain grouping metadata |
| `filter(expression)` | Keep rows whose expression is truthy |
| `derive({name: expression})` | Add or replace columns; every expression sees the original input |
| `groupby(...names)`, `groupby({alias: expression})` | Set group keys without aggregating; preserve first-occurrence group order |
| `ungroup()` | Remove grouping metadata |
| `rollup({name: expression})`, `count({as})` | Compute aggregate output for each group, or globally |
| `orderby(...keys)`, `desc(key)` | Stable multi-key ordering, numeric or well-formed Unicode string keys |
| `slice(start, end)` | Half-open logical row slice, including negative bounds |
| `join`, `join_left`, `join_right`, `join_full` | Equijoins with multiplicity, nullish/NaN nonmatching keys, natural or explicit keys |
| `semijoin`, `antijoin` | Keep left rows with/without a right match |
| `objects()`, `array(name)`, `get(name, row)` | Materialize JS values or inspect a logical row |
| `numRows`, `numCols`, `columnNames`, `isGrouped` | Inspect table metadata |

Join keys use `"key"`, `["leftKey", "rightKey"]`, or
`[["a", "b"], ["c", "d"]]`. Omit keys for a natural join. Shared key columns
are combined; colliding non-key names use `_1` / `_2`, overridable with
`join(other, keys, null, {suffix: ["_left", "_right"]})`. Ungroup before joining;
custom predicate joins and join projection expressions are not supported.
Both tables must belong to the same instantiated module.

## Scalar and expression contract

Cells support JS numbers (f64, including signed zero, NaN and infinities), strings,
booleans, null, and undefined. The JS facade's input columns use one non-null scalar type.
Numbers do not implicitly become i32. BigInt, Date, arrays, objects, Symbols and
malformed UTF-16 strings are rejected. Exact string equality does not normalize
Unicode. String sorting follows UTF-16 code-unit order for well-formed strings,
including astral characters; locale collation is not implemented.

Expressions are synchronous arrows/functions with at most one row parameter,
static field references (`d.x`, `d["x"]`), simple destructuring, literals,
arithmetic `+ - * /`, comparisons, `&&`, `||`, `??`, unary `! - +`, and ternary
conditionals. Boolean operators and conditionals short-circuit in Wasm. A block
body may contain one return expression. Free variables, arbitrary function calls,
effects, async expressions, assignment, loops, and unsupported syntax are rejected.
These are table expressions represented as data, not general JS callbacks.
Columns are bound before row evaluation, so invalid references and aggregate
placement are rejected even on empty inputs and in unexecuted branches.
Supported aggregate calls resolve by their registered method name, including
minified namespace aliases. The namespace is declarative syntax, not a captured
JS object; its host methods are never invoked. Arguments must still be valid
table expressions. Calls on row values and unregistered methods are rejected.
Expression source strings are not accepted; write `() => "literal"` for a string
constant, and use arrows for computed expressions.

`+` supports numeric operands or two strings. Numeric operations support numbers,
booleans and nullish numeric conversion. General JS string/number coercion is
outside this version's contract. Use strict equality for ordinary comparisons;
`== null` / `!= null` also handle undefined. There is no regex/string-method API.

Aggregates are `op.count`, `sum`, `mean`/`average`, `min`, `max`, `valid`,
`invalid`, and `distinct`. Use descriptors such as `op.sum("amount")` or arrows
such as `d => op.sum(d.amount * d.units)`. Aggregates are supported in rollup;
nested aggregates and aggregate/window expressions in other verbs are rejected.
Count includes all rows. Numeric aggregates skip null, undefined, and NaN;
an empty/all-invalid numeric aggregate returns undefined. Distinct collapses
invalid values into one bucket, while grouping keeps null, undefined, and NaN
as separate keys. Grouping treats -0 and +0 as the same key.
In rollup, column references must be inside an aggregate; constant expressions
and arithmetic over aggregate results are also supported.
TypeScript distinguishes row expressions from aggregate descriptors and infers
scalar column/aggregate results. Inline aggregate returns are typed. For typed
arithmetic combining aggregates, roll up to named fields and use a subsequent
`derive`; arithmetic directly on descriptor objects is not part of the TS API.

## Native and portable boundaries

[examples/table/main.walz](../examples/table/main.walz) uses the same engine from
Walz and returns 42. Native expression helpers include `$expr.number`, `$expr.string`,
`$expr.column`, arithmetic/comparison/Boolean builders, and named aggregate builders.
The low-level `$expr.node` protocol is used by the JS adapter; normal Walz code
can use named builders. Table storage is opaque and effect bounds remain checked.

[examples/table/portable.walz](../examples/table/portable.walz) exports `run(input,
commands)` through Core + JSPI and Component value adapters. It accepts an explicit
row count and named columns of `{kind, number, text}` records, plus topologically
ordered expression programs and transformation commands. It executes a whole
pipeline inside one call and returns a fresh snapshot of visible rows; grouping
metadata is local to the pipeline. This finite wire format can represent mixed
tagged cells for transport even though the JS fluent facade accepts homogeneous
input columns. Normal query operations still follow the scalar contract above.

Expression nodes may reference only earlier nodes, preventing cycles. Unknown
opcodes, invalid schema lengths, bad Boolean tags, bad column references, and
invalid aggregate placement trap. The [portable test](../runtime/table-portable.test.mjs)
and its [encoding helpers](../runtime/testing/table-wire.mjs) show the wire format.
These snapshot calls do not provide persistent Component handles or transfer GC
references across agents.

## State, lifetime, and boundaries

Tables, columns, expressions, and cells are checked GC objects. Transformations
return new tables. Select/filter/order operations share immutable columns;
retaining a small selection can therefore retain its larger backing columns.
Derived columns and joins allocate new values. Mutating the original JS arrays
after ingestion does not change the table.

The bulk Core facade stores numeric/Boolean/nullish input in immutable f64 and
packed-tag lists, rather than allocating a Cell for every input element. The
payload is `8 * physicalRows + 4 * ceil(physicalRows / 4)` bytes per column,
excluding GC/list headers. Publishing copies the mutable builder; subsequent
builder writes cannot change the column. Strings and portable wire snapshots
continue to use Cells.

Native `$column.get` and `$column.values` materialize canonical Cells lazily.
Repeated accesses share Cell identity, including accesses made before a full
list view. The first individual access allocates a nullable reference array of
column length; a full view replaces that cache with an immutable Cell list.
These native operations, and `$table.get`, now require `reads writes allocates`
effects. This private cache mutation does not change logical values or the JS
API. Bulk numeric output reads the primary storage without materializing Cells.
Scalar expression evaluation, sorting, grouping and joins can still create them;
retaining a full view can retain both representations until GC collects the column.

Numeric filtering additionally attaches a lazy f64x2/type-mask cache, with payload
`20 * ceil(physicalRows / 2)` bytes per prepared column. It has no references to
Cells, tables or JS sources. Scalar mode avoids this additional cache. Packing
is a speed/memory tradeoff, not a reduction in primary storage size.

The Core facade passes opaque GC references directly to exports of the same
instance. It stores no numeric-handle registry and needs no `close()` protocol.
Numeric/Boolean/nullish columns use a bounded linear transfer buffer in batches
of at most 65,536 rows. It starts at one Wasm page and grows to at most ten pages
(640 KiB including the private header and page rounding), independent of table
length and call count. Table data stays in GC objects. Each synchronous transfer
closes its borrow in `finally`, including after a trap; published columns and
returned JS arrays do not retain or alias this scratch buffer. Strings use the
generic cell bridge. Big-endian hosts use the scalar bridge.
`objects()` creates ordinary independent JS objects using a shared property
template. It preserves own data properties such as `__proto__` and bypasses
inherited setters, without generating or evaluating code. Bulk output fills
these rows directly, without allocating intermediate JS column arrays. Columnar
consumers can use `array(name)` to avoid row-object construction.
Input chunks use local typed staging arrays. Getters run once before the Wasm
scratch borrow, so reentrant input getters remain supported. This adds bounded
JS staging of at most 65,536 rows per chunk; it does not retain the source arrays.
Table wrappers retain the instance and the values they use, not their JS input
arrays or parent wrappers. Drop references to let GC reclaim them. GC references
cannot be sent to another Worker or passed as Component values.
The fluent facade rejects malformed UTF-16 before encoding. The portable snapshot
boundary follows the runtime's existing UTF-8/WIT conversion behavior, including
replacement of unpaired JS surrogates; use well-formed host strings for exact
comparisons with Arquero.

JS parses expressions and converts data at ingestion/materialization. Selection,
derived-row evaluation, grouping, aggregation, sorting, and hash joining execute
in Walz.

## SIMD numeric filters

The default `filter` path recognizes comparisons between a column and a numeric
literal (`< <= > >= === !== == !=`), including a literal on the left, unary
signs, and `&&`, `||`, `!` compositions. It packs numbers, booleans, null and
undefined into f64x2 lanes, keeping separate type masks for strict and loose
equality. NaN, signed zero, infinities, null-to-zero ordered comparisons, and
Boolean-to-number conversion keep their existing behavior. Two adjacent physical
rows use one block load; sparse or reordered rows gather the two selected lanes.
The final lane is masked. Logical row order and grouping order are unchanged.

The whole expression is checked before a scan, including on empty input. Only
total numeric comparisons and literal truth values may use lane-mask composition.
AND skips its right subtree when the left pair mask is zero; OR skips it when
both bits are one. Same-column pairs of ordered comparisons are fused: load or
gather once, combine vector masks with AND/OR, then convert to a bitmask once.
This covers common range and disjoint-range conditions; other supported numeric
trees retain the checked general plan. Packed scalar comparisons remain separate.
An unsupported expression or a referenced column containing strings uses the
original scalar evaluator, preserving short-circuiting and executed-branch traps.
Arithmetic, column-to-column comparisons, and aggregates are not vectorized.
Floating-point sums and means retain their scalar iteration order.

For investigation or memory-sensitive callers, select an execution mode:

```js
await instantiate(base, { filter: "simd" });          // default, lazy packed cache
await instantiate(base, { filter: "scalar" });        // original cell evaluator
await instantiate(base, { filter: "packed-scalar" }); // same cache, scalar comparisons
await instantiate(base, { io: "scalar" });            // per-cell bridge control
```

The packed scalar control still uses the same v128 storage/lane operations; only
comparisons are scalar. All modes track whether base membership and logical
order are identical, scanning once when they are and independently otherwise.
Numeric filters store private pair masks, allocate the final row-ID array at its
exact selected length, and publish it through the normal immutable list snapshot
constructor. This removes the selection-prefix allocation/copy while preserving
the public constructor's copy semantics. Scalar fallbacks retain generic evaluation.
Native equivalents are `$table.filter`, `$table.filter-scalar`, and
`$table.filter-packed-scalar`; portable snapshots expose `run`, `runScalar`, and
`runPackedScalar` with the same wire schema.

```sh
just bench-table benchmarks/table/new-run
```

The harness pins official Arquero 8.0.3, saves fixed seeded numeric/nullable
inputs, sources, source hashes, Wasm and raw timings to a new directory. It
compares resident filters, fresh ingestion plus first filter, and filtering plus
JS object materialization. A direct JS count scan is a lower-bound control that
does not allocate a filtered table or row-ID selection. Full rows are verified
outside timing, with nine rotating-order samples and GC outside clocks. Run
without other builds/tests. Report cold and output costs separately from resident
filter gains, and compare SIMD with packed scalar to isolate comparison effects.
See the [captured runs](../benchmarks/table/README.md) for results and limits.
The [five-stage tuning matrix](../benchmarks/table/tuning/20261113-r1/README.md)
and [after profile](../benchmarks/table/profiles/20261113-r2/README.md) record the
bulk bridge, aligned scan, mask short-circuit, fusion and selection-allocation changes.

The [input/output suite](../benchmarks/table/io/20261113-r1/README.md) times `table(data).filter(predicate).objects()` from
existing JS column arrays through ordinary JS result rows, plus isolated input,
resident scan and output phases. It includes copies, query construction and GC,
but excludes module loading, JSON serialization and network I/O. Run captured
before/after artifacts together with `just bench-table-io <baseline-capture>
<candidate-capture> <new-output-directory>`; use a new directory for every run.

To inspect CPU samples, sampled allocations and export-call counts on a captured
artifact, use `just profile-table <capture> <new-output-directory>`. This does not
rebuild the current source. The [profile report](../benchmarks/table/profiles/20261113-r1/README.md)
records remaining optimizations and separates observations from work-count models.
`just probe-table-wasm-opt <capture> <new-output-directory>` tests optional
Binaryen postprocessing separately; it does not change the regular build.

## Verification

```sh
just test-table
pnpm exec playwright test runtime/e2e/table.spec.mjs
```

The tests compare complete results with official Arquero 8.0.3, exercise the
native Walz library on Wasmtime, check Core + JSPI and Component pipelines and
trap cleanup, and verify GC reachability, immutable snapshots, and TypeScript
inference/rejection. Chromium tests run both the bundled resident facade and
portable snapshots. These are functional checks, not speed measurements.

## Compatibility work remaining

Window operations, sampling, pivot/fold/spread/unroll, set operations, general
custom functions, arbitrary JS values/coercion, locale collation, Arrow/CSV/JSON
I/O, full projection helpers, and the complete aggregate function catalogue
remain outside this first version. See the maintained TODO for the broader
roadmap. Unsupported inputs are rejected rather than delegated to Arquero.
