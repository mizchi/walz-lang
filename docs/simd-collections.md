# SIMD collections: bitmaps and i32 columns

Walz's first reusable SIMD collections are an immutable dense bitmap and an
immutable signed i32 column. They are ordinary Walz libraries in `walz/simd/`,
with private Wasm GC `array (mut v128)` storage. They require no new syntax,
compiler instructions, host imports, linear-memory allocator, or runtime table.

[Dictionary columns](dictionary-column.md) build on the i32 column and bitmap
APIs, providing a unique string dictionary, validated IDs, decoding and selections.

## Choosing a useful representation

| Structure | Useful work | SIMD representation | Main cost or contract |
| --- | --- | --- | --- |
| Dense bitmap, implemented | Compose filters, count intersections, select row IDs | 128 bits per GC array element | Logical length and canonical zero padding; sparse sets may waste space |
| i32 column, implemented | Equality/range filters and exact sums | Four signed i32 values per element | Copy once, retain for repeated queries; `i64` widening before sums |
| f32 vectors, next candidate | Dot products, squared distances, fixed-width numerical batches | Four f32 values per element | Specify reduction order, rounding, NaN and infinity behavior |
| Byte-block index | Repeated byte classification/search, parser indexes | Sixteen bytes per element and scalar bitmasks | Packing costs; a one-pass parser may benefit more from direct byte access |
| Adaptive or hashed indexes | Sparse sets or repeated key lookup | Dense containers or control-byte groups | More invariants: container choice, hashing, collisions, resizing and publication |

The first two compose into a small analytical pipeline without floating-point
reassociation or mutable hash-table invariants. Columnar values and separate
selection/validity bitmaps are also established patterns in the
[Arrow format](https://arrow.apache.org/docs/format/Columnar.html). These
collections do not implement Arrow buffers, IPC, null semantics, or its ABI.

The existing [succinct indexes](succinct.md) cover rank/select and wavelet
queries. These collections add Boolean selection and fixed-width column scans;
keep an index when its indexed queries fit the workload.

[Wasm SIMD](https://github.com/WebAssembly/spec/blob/main/proposals/simd/SIMD.md)
provides lane comparisons, bitmask extraction, bitwise operations, and byte-lane
popcount. The library combines these operations rather than adding an abstract
SIMD type system to the language. Sparse compression remains a different choice;
see [Roaring's container format](https://github.com/RoaringBitmap/RoaringFormatSpec).

## A complete query

```wat
(module
  (@use "walz:core" (type $list))
  (@use "../../walz/simd/bitmap.walz" (func $bitmap.and))
  (@use "../../walz/simd/i32-column.walz"
    (func $i32-column.from-list) (func $i32-column.between)
    (func $i32-column.eq) (func $i32-column.sum-where))
  (func $main (export "main") (result i64) (@effect reads writes allocates)
    (@let (($prices (call $i32-column.from-list (@list.new i32 10 20 22 70)))
           ($categories (call $i32-column.from-list (@list.new i32 1 2 2 1)))
           ($selected (call $bitmap.and
             (call $i32-column.between $prices 10 30)
             (call $i32-column.eq $categories 2))))
      (call $i32-column.sum-where $prices $selected))))
```

Save at `examples/simd-collections/main.walz` and run
`walz run examples/simd-collections/main.walz`: rows 1 and 2 match, producing 42.
Adjust relative library paths for your project. This is a source library, not
another implicit `walz:core` import or a package-resolution scheme.

Keep columns and frequently used selections in Walz when issuing multiple
queries. `count-eq` and `count-between` fuse a single predicate and count without
allocating a bitmap. Materialize a bitmap when it will be combined, reused,
enumerated, or passed to `sum-where`.
Selections combine by row position; callers align the row ordering of their
columns. Equal lengths do not establish that two columns describe the same data.

## Bitmap contract

Import from `walz/simd/bitmap.walz`, selecting the required names.

| API | Contract |
| --- | --- |
| `$bitmap.empty(n)`, `$bitmap.full(n)` | Logical length `n`, all bits zero or one |
| `$bitmap.from-words(n, list<i32>)` | Copy exactly `ceil(n/32)` words; clear all padding bits |
| `$bitmap.len`, `$bitmap.word-len` | Logical bit length and logical word count |
| `$bitmap.word(b, i)`, `$bitmap.get(b, i)` | Signed i32 word bits, or scalar 0/1 at a bit index |
| `$bitmap.to-words`, `$bitmap.indices` | Independent list of words, or increasing selected row IDs |
| `$bitmap.and`, `$bitmap.or`, `$bitmap.xor`, `$bitmap.and-not` | New bitmap; operands must have identical logical lengths |
| `$bitmap.not` | New bitmap with only its logical bits inverted |
| `$bitmap.count`, `$bitmap.intersection-count`, `$bitmap.intersects` | Scalar results; no intermediate result bitmap |

Lengths are nonnegative signed i32 values. Negative lengths, wrong word counts,
invalid indices, and unequal logical lengths trap. An index at the logical end
is invalid even if its physical vector contains padding. Bit `i` is bit
`i % 32`, starting from the least significant bit, in word `i / 32`. A negative
i32 word represents its ordinary 32-bit pattern. High input bits outside the
logical length are masked rather than rejected.

Storage contains `ceil(n/128)` vectors. Full and complemented bitmaps mask the
final block; binary operations preserve this invariant. Popcount uses
`i8x16.popcnt` followed by unsigned widening pairwise additions and a scalar
reduction. `intersects` exits at the first nonzero intersection block.

## Column contract

Import from `walz/simd/i32-column.walz`.

| API | Contract |
| --- | --- |
| `$i32-column.from-list(list<i32>)` | Independent snapshot, with zero-filled final lanes |
| `$i32-column.len`, `$i32-column.get` | Logical row count and bounds-checked signed i32 access |
| `$i32-column.to-list` | Independent scalar list |
| `$i32-column.eq(column, value)` | Selection bitmap for equality |
| `$i32-column.between(column, lower, upper)` | Signed half-open range `[lower, upper)` |
| `$i32-column.count-eq`, `$i32-column.count-between` | Fused counts with no intermediate bitmap |
| `$i32-column.sum` | Exact signed i64 sum |
| `$i32-column.sum-where(column, bitmap)` | Exact signed i64 sum for a same-length selection |
| `$i32-column.query-in-between(codes, amounts, needles, lower, upper)` | One fused IN/range scan returning immutable `$i32-query-result` fields `$count`, `$sum`, `$indices` |

Fused queries require aligned, equal-length columns, including for empty key
lists and reversed ranges. Duplicate keys do not duplicate rows. The result
contains ordered row IDs and an exact i64 sum, retains no input column, and
creates no selection bitmap. See [fused dictionary queries](dictionary-column.md#fused-row-queries)
for the string-key adapter, result type, and examples.

Equal or reversed range limits select no rows. No sentinel value is reserved:
`i32.MIN`, `i32.MAX`, and zero are ordinary data. Comparisons never select padded
rows, including zero padding when comparing with zero. Sums widen each lane
before addition, so no i32 subtotal can overflow. The exact sum fits in i64 for
every supported logical length.

## Effects, ownership, and interoperability

The exported nominal types are opaque. Walz callers cannot construct them or obtain
their mutable storage. Constructors and set operations allocate fresh objects;
there is no public setter or hidden mutation of an existing collection.
Reads still carry `@effect reads` because the implementation uses mutable GC
arrays for private construction; no effect-checking exception is added for this
library. Check each function's declaration for its exact effects.

GC manages the objects. No explicit close or task cancellation is needed for a
completed synchronous query. Tests retain a derived bitmap while verifying that
its source bitmap, column, and input array are collectible. A live column or
bitmap intentionally retains its own storage.

These are agent-local GC objects, not references shared between Workers. Use
scalar lists/words to copy a snapshot to another agent. A future zero-copy
shared-memory representation must use the existing readonly-region publication
and joined Worker lifetime contracts; it cannot publish a GC reference.

Direct `v128` values cannot cross the JS or Component value ABI. The
[portable example](../examples/simd-collections/portable.walz) accepts scalar
lists and returns scalars/lists or an explicitly annotated WIT record. The
[resident example](../examples/simd-collections/resident.walz) instead exposes
opaque GC objects through standalone Core Wasm. It is a separate interface,
without a portable WIT representation of the collection objects themselves.

For portable callers, `api.countMany(values, lowerLimits, upperLimits)` packs
one column and evaluates all half-open ranges in order. Limits must have equal
lengths; an empty batch returns an empty list. The result is an `Int32Array`.
This amortizes transfer and packing without a persistent handle table:

```js
api.countMany(Int32Array.of(10, 20, 22, 70),
  Int32Array.of(10, 20, 70), Int32Array.of(30, 22, 70));
// Int32Array [3, 1, 0]
```

## Verification and measurement

```sh
just test-simd-collections
pnpm exec playwright test runtime/e2e/simd-collections.spec.mjs
just bench-simd-collections target/simd-collections-bench/new-run
```

Tests compare independent scalar oracles at lane, 32-bit word and 128-bit block
boundaries; include empty inputs, invalid lengths, signed extremes, padding,
snapshots, GC reachability, ABI allocation cleanup, and traps. The native test
runner uses Wasmtime; Core + JSPI and Component execute under Node and Chromium.
All library functions remain in the checked typed core. No new Lean contract
or claim of formal end-to-end SIMD equivalence is introduced.

The benchmark accepts only a new output directory. Resident and copy-inclusive
calls are separate experiments. Version 4 rotates nine samples among SIMD
Walz, scalar Walz, and JS controls; range and sum controls include branchless JS
as well as branching loops. Fixed seeded inputs, sources, compiler hashes,
executed Wasm/glue, setup costs, environment, and raw samples are saved.
Packing and startup are outside resident timings and inside one-shot timings.
Results are checked outside clocks, ABI tables must be empty after each
portable sample, and explicit GC stays outside clocks. Automatic GC during
timing is included.

The portable batch workload executes 32 ranges per transfer and checks every
returned count. It warms at least 32 batches and 120 ms per variant; the single
query workloads warm at least 1,024 calls and 120 ms. Use the batch duration
when estimating throughput: it is not the cost of a single range query.

Use the fastest applicable JS control when assessing a replacement. Faster
resident queries alone do not establish a benefit for one-shot calls, sparse
inputs, a different engine, or a full application. Keep previous measurements
unchanged and create a new suite version when changing the cases or controls.

## Initial measurements

Apple M5, Node 24.22.2 / V8 13.6, 2026-11-13 JST. At 262,144 rows or logical
bits, medians in microseconds:

| Resident operation | Fastest JS control | Scalar Walz | SIMD collection |
| --- | ---: | ---: | ---: |
| Bitmap intersection count | 7.543 | 3.556 | 1.442 |
| Signed range count | 150.447 | 95.838 | 52.833 |
| Selected exact sum | 163.767 | 149.827 | 78.995 |

SIMD collections improve these resident queries by 2.07–5.23x over the fastest
JS control. The comparison with scalar Walz is smaller because moving the loop
into Wasm already helps; see the [complete record](../benchmarks/simd-collections/20261113-r4/README.md).

A single portable range query takes 359.527 µs in Core + JSPI, compared with
155.478 µs in branchless JS. For **32 ranges in one transfer**, branchless JS
takes 4.479 ms, Core + JSPI 1.984 ms (2.26x faster), and Component 2.056 ms
(2.18x faster), including copies, packing, and all returned counts. This is the
useful initial workload: reuse packed columns inside a computation or batch
enough queries to amortize their construction. These numbers concern the
captured dense integer data and this engine, not arbitrary JS libraries.
