# Dictionary-encoded string columns

`walz/simd/dictionary-column.walz` represents a string column as a unique,
immutable dictionary plus an existing SIMD i32 code column. It is intended for
repeated queries over low-cardinality values such as countries, categories, and
status labels. No language syntax, host imports, runtime registry, or new
ownership rule is added.

```text
values:     ["JP", "US", "JP", "FR"]
dictionary: ["JP", "US", "FR"]
codes:      [0,    1,    0,    2]
```

Construction uses the existing string-keyed hash dictionary and list builder.
Only the immutable keys and packed code column survive publication. Queries
resolve a string against the small dictionary, then compare row IDs using the
existing `i32x4` operations. The initial representation uses four i32 lanes per
vector; u8/u16 packing and nullable columns are separate [roadmap tasks](../TODO.md#simd-library-roadmap).

## Walz use

```wat
(module
  (@use "walz:core" (type $string) (type $list))
  (@use "../../walz/simd/dictionary-column.walz"
    (func $dictionary-column.from-list) (func $dictionary-column.eq))
  (@use "../../walz/simd/i32-column.walz"
    (func $i32-column.from-list) (func $i32-column.sum-where))
  (func $main (export "main") (result i64) (@effect reads writes allocates)
    (@let (($countries (call $dictionary-column.from-list
             (@list.new $string "JP" "US" "JP" "FR")))
           ($amounts (call $i32-column.from-list (@list.new i32 20 100 22 3))))
      (call $i32-column.sum-where $amounts
        (call $dictionary-column.eq $countries "JP")))))
```

This is [examples/dictionary-column/main.walz](../examples/dictionary-column/main.walz).
Run it from the repository root with `walz run examples/dictionary-column/main.walz`;
the result is 42. Adjust relative library paths for an application elsewhere.

## API and invariants

| Function | Behavior |
| --- | --- |
| `$dictionary-column.from-list(list<string>)` | Encode using IDs assigned in first-occurrence order |
| `$dictionary-column.from-codes(dictionary, codes)` | Validate a supplied codebook and retain its order |
| `$dictionary-column.len` | Logical row count |
| `$dictionary-column.dictionary-len` | Number of dictionary entries, including unused supplied entries |
| `$dictionary-column.dictionary` | Immutable dictionary list |
| `$dictionary-column.codes` | Independent list of scalar i32 codes |
| `$dictionary-column.code-column` | Immutable opaque i32 column for explicitly code-based operations |
| `$dictionary-column.get(column, row)` | Decode one bounds-checked row |
| `$dictionary-column.decode` | Decode all rows to an immutable string list |
| `$dictionary-column.find-code(column, value)` | `option<i32>`: dictionary-local ID or none |
| `$dictionary-column.eq(column, value)` | Selection bitmap; an absent value produces all-zero bits |
| `$dictionary-column.in(column, values)` | Union of selections; duplicate query values never duplicate rows |
| `$dictionary-column.count-eq` | SIMD count after dictionary lookup |
| `$dictionary-column.count-many` | Counts in query order, reusing the constructed column |
| `$dictionary-column.query-between(column, amounts, keys, lo, hi)` | Fused IN and signed half-open range query, returning count, exact i64 sum, and ordered row IDs |

Every stored ID is nonnegative and less than the dictionary length. Explicit
codebooks must have unique strings, even when their codes list is empty. Negative
or out-of-range IDs, duplicate supplied keys, and invalid row indices trap.
Unused unique dictionary entries are allowed; their count is zero.

Equality compares exact UTF-8 bytes. Empty strings, NUL, BOM, and Unicode are
ordinary values. There is no Unicode normalization or missing-value sentinel.
The JS UTF-8 ABI retains its existing replacement of unpaired UTF-16 surrogates;
the uniqueness check applies to the resulting strings. Nullability is not part
of this initial API.

IDs belong to their dictionary: code zero in two independently built columns
need not name the same string. Serialize the dictionary and codes together.
Use value-based selections when combining different dictionaries. Selection
bitmaps still combine by row position; callers align their columns' row ordering.

The column's nominal type is opaque, and its published fields have no setters.
Dictionary lookup is pure and linear in dictionary size; row scans read private
SIMD storage. Constructors, snapshots and selections declare their allocation
and mutation effects. See function declarations for exact effect bounds.

This design targets small dictionaries. High cardinality can make linear query
lookup and construction expensive. The dictionary benchmark below measures this
library separately from the bitmap and numeric-column benchmarks.

## JS and Component values

The [portable example](../examples/dictionary-column/portable.walz) exposes only
scalar lists and explicitly annotated WIT records:

```js
const encoded = api.encode(["JP", "US", "JP"]);
// { dictionary: ["JP", "US"], codes: Int32Array [0, 1, 0] }
api.decode(encoded);                          // ["JP", "US", "JP"]
api.select(["JP", "US", "JP"], ["JP"]);      // Int32Array [0, 2]
api.countEncoded(encoded, ["JP", "US", "?"]); // Int32Array [2, 1, 0]
```

`countMany(values, queries)` encodes once for the whole query batch.
`countEncoded(encoded, queries)` validates and packs supplied codes once per
batch, avoiding a full string-column reconstruction. Both include ordinary ABI
copies. Returned JS arrays are independent; modifying them does not mutate a
previous guest column. No persistent JS handle table is used, and GC references
are not sent between Workers.

## Fused row queries

`$dictionary-column.query-between` resolves keys once, then delegates to
`$i32-column.query-in-between(codes, amounts, needles, lo, hi)`. Both return a
shared `(ref $i32-query-result)`; import that type from `i32-column.walz` to read
its immutable `$count: i32`, `$sum: i64`, and `$indices: list<i32>` fields.
The numeric helper accepts arbitrary signed i32 keys. Neither function changes
the language, the GC ownership model, or the host interface.

The query scans packed codes once, skips amount blocks with no matching category,
combines the range predicate with the category mask, and widens before summing.
It emits ascending row IDs into a private growing buffer and publishes only a
fresh immutable list. No intermediate bitmap or persistent index is constructed;
the result retains neither input column. This is useful when rows and aggregates
are needed together. Use the existing bitmap functions when reusing selections
or composing them with other predicates.

Duplicate keys form a union and do not duplicate rows or sums. Missing keys,
empty key lists, empty aligned columns, and `lo >= hi` produce a zero count/sum
and empty indices. Column-length mismatch **always traps**, including those
otherwise-empty queries. Callers still align row ordering. Padding lanes are
excluded, and the exact signed sum fits i64 for every supported column length.
Many query keys still require repeated comparisons within each block; no hash
lookup, histogram, or postings index is stored in the column.

The portable example's `api.query` uses this path. `api.queryComposed` retains
the earlier bitmap composition as an executable correctness and timing control.
The [standalone fused example](../examples/dictionary-column/fused.walz) returns
42; run `walz run examples/dictionary-column/fused.walz` from the repository root.

## Measured performance

### Fused row queries (suite version 2)

The [paired fusion benchmark](../benchmarks/dictionary-column/20261113-r3/README.md)
times the old and fused query within the same process/module, returning all row
IDs, count, and exact sum. With 262,144 resident rows, 64 ASCII labels, one present
key, and the amount range `[-500, 500)`, the primary medians are:

| Implementation | Time per complete query |
| --- | ---: |
| Original bitmap composition | 209.691 µs |
| Fused Walz query | 85.985 µs |
| Faster JS code scan | 233.081 µs |
| Retained JS postings | 8.643 µs |

Fusion improves the original by 2.44x and beats JS code scanning by 2.71x in this
case. An [independent run](../benchmarks/dictionary-column/20261113-r4/README.md)
reproduces 209.232 → 84.687 µs (2.47x), with identical executable snapshots and
artifacts. Across the captured matrices, gains over composition range from
1.11x to 2.95x; dense four-label data improves less. Sparse JS postings remain
substantially faster. Multi-key semantics are tested, but the query timing uses
one key. This does not establish arbitrary-query or browser speedups.

Raw-string ABI calls still take about 66 ms for Core and 78 ms for Component in
this case, with no established gain from fusion. Construction and conversion
must be addressed separately. No histogram, postings index, or stable persistent
host-handle API was added, and count-only functions are unchanged.

### Baseline before query fusion (suite version 1)

The [captured dictionary benchmark](../benchmarks/dictionary-column/README.md)
compares construction, reused queries, and complete Core + JSPI / Component calls.
This table records the original bitmap-based query, before fusion. Count-only
functions remain unchanged. The medians are from Apple M5 / Node 24.22.2, with 262,144 rows and 64
distinct ASCII labels. Times are per whole operation; a count batch has **32 queries**.

| Operation | JS | Walz | Scope |
| --- | ---: | ---: | --- |
| Encode strings | 2.908 ms | 4.072 ms | Input strings already resident in each engine |
| Validate and pack supplied codes | 0.824 ms | 0.222 ms | Resident; unique-key and ID validation included |
| 32 code-scanning counts | 3.725 ms | 1.436 ms | Resident; JS uses a retained key-to-code Map |
| 32 counts with a retained histogram | 0.00135 ms | 1.436 ms | JS uses a different, counts-only index |
| One category/range query, returning rows and sum | 0.228 ms | 0.213 ms | Resident; JS scans codes, Walz composes selection bitmaps |
| Same query with retained JS postings | 0.00807 ms | 0.213 ms | JS scans only the selected category's row IDs |
| 32 validated-code counts through the value ABI | 0.863 ms | 1.880 / 1.932 ms | JS validates and builds a histogram; Core / Component include all copies |
| Same batch from raw strings through the value ABI | 4.636 ms | 70.586 / 84.663 ms | JS builds a histogram; Core / Component encode once per batch |

The resident scalar-Wasm control takes 2.473 ms for 32 counts. Thus SIMD with its
packed layout is 1.72x faster than scalar Wasm and 2.59x faster than the JS code
scan here; the larger gain over string scanning also includes dictionary encoding.
This is a conditional scan advantage. A histogram answers equality counts without
rescanning rows, and postings avoid full scans for sparse selections. Neither
index is retained by the current dictionary-column representation.

For 4,096 rows and 1,024 labels, linear dictionary lookup makes Walz counts slower
than JS with a retained Map. Row-returning queries also have no consistent advantage
over the strongest JS controls. Sending a raw string column across the ABI on every
call is particularly expensive. Preencoded values reduce that cost, but do not beat
the validated JS histogram in this suite. Reuse columns inside a guest computation
when appropriate; the resident fixture's GC-reference exports are measurement tools,
not a persistent handle API for Component values or Workers.

A second independent process reproduced the resident count-scan advantage at
2.61x over JS code scans and 1.73x over scalar Wasm, with the same inputs and
executed artifacts. It also reproduced the losses against JS indexes and the
cost of raw-string ABI calls. The linked comparison keeps both runs separate.

```sh
just bench-dictionary-column target/dictionary-bench/new-run
```

The output directory must be fresh. The suite checks every code, count, returned
row ID, and sum outside the clock, consumes every timed result, and checks zero
live ABI allocations and boundary tables after each portable sample. It includes
4,096 / 65,536 / 262,144 rows, 4 / 64 / 1,024 labels, and a Unicode case. See the
captured report for raw samples, input files, setup observations, controls, source
hashes, and executed artifacts. Browser performance and other query distributions
have not been measured by this suite.

## Verification

```sh
just test-dictionary-column
pnpm exec playwright test runtime/e2e/dictionary-column.spec.mjs
```

Native tests exercise first-occurrence IDs, zero IDs, UTF-8 equality, empty and
unused dictionaries, IN semantics, and batched counts. Independent JS Map/array
oracles check code order and decoding, vector/bitmap tails, hash-table growth,
IDs above 255 and 65,535, and strict validation. Both stable JS backends check
zero live ABI allocations/tables after results and traps. Chromium checks the
same value adapters and composition with numeric column filters.

Fused queries are checked against both the bitmap composition and independent
scalar/BigInt oracles, including dense output growth, duplicate and missing keys,
signed extrema, exact sums beyond i32, empty/reversed ranges, and length traps.

Forced-GC tests keep a live column after discarding its source container, then
retain only a dictionary, only a code column, or only a selection. Unneeded
owners must be collectible while the retained value remains usable. Canonical
dictionary strings remain live while their dictionary is held; the library
does not copy immutable string payloads or add a close/dispose protocol.
Additional reachability checks keep only a fused result or its row list, requiring
the source containers, both columns, and any discarded result wrapper to collect.
