# SIMD collections

English | [日本語](README.ja.md)

Immutable Wasm GC collections for repeated bulk queries:

- `bitmap.walz`: 128-bit blocks, Boolean set operations, count/intersection,
  and selected row IDs.
- `i32-column.walz`: four i32 lanes per block, equality/range selections,
  fused counts, exact i64 sums, and fused IN/range row queries.
- `dictionary-column.walz`: immutable string dictionary and validated i32
  codes, value-based selections, decoding, batched counts, and fused numeric queries.

Use explicit relative `@use` paths and import only the types/functions needed.
These libraries add no language syntax, host dependency, or runtime table.
They are separate from the `walz:core` facade.

See [contracts and examples](../../docs/simd-collections.md),
[dictionary columns](../../docs/dictionary-column.md),
[the complete query](../../examples/simd-collections/main.walz), and
[the JS/Component value adapter](../../examples/simd-collections/portable.walz).

```sh
walz run examples/simd-collections/main.walz
just test-simd-collections
just bench-simd-collections target/simd-collections-bench/new-run
just test-dictionary-column
just bench-dictionary-column target/dictionary-bench/new-run
```

Packing copies data. Retain packed collections for multiple queries; measure
ABI conversion too when deciding whether to replace JS. Internal `v128` values
do not cross the WIT/JS ABI, and GC references are not shared between Workers.
Dictionary scans have a measured resident advantage over JS code scans; retained
JS histograms/postings and raw-string ABI costs change that conclusion. See the
[dictionary measurements](../../docs/dictionary-column.md#measured-performance).
