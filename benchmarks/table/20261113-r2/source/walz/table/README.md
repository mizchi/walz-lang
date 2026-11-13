# Table processing

English | [日本語](README.ja.md)

Immutable column-oriented tables implemented in Walz, with an Arquero-style
JS/TS facade. Core verbs cover selection, filtering, derived columns, grouping,
aggregation, stable ordering, slicing, and equijoins. Cells use f64 numbers,
well-formed strings, booleans, null, and undefined.

- `value.walz`: opaque scalar cells, equality, ordering, and grouping keys.
- `engine.walz`: opaque tables/columns/expressions and transformations.
- `number-column.walz`: lazy f64x2 numeric filter cache with explicit type masks.
- `join.walz`: hash joins, duplicate matches, outer joins, semi/anti joins.
- [`runtime/table/`](../../runtime/table/): JS expression parser and persistent
  Core facade, passing GC references within one instance without a handle registry.
- [`portable.walz`](../../examples/table/portable.walz): one-shot pipeline
  snapshots for Core + JSPI and Component value adapters.

```sh
just build-table
node examples/table/demo.mjs
just test-table
just bench-table benchmarks/table/new-run
```

Use explicit relative `@use` paths for native Walz code; see the
[native example](../../examples/table/main.walz). The [table guide](../../docs/table.md)
defines exact APIs, expression rules, lifetimes, and compatibility limits.
Numeric constant comparisons and Boolean compositions use SIMD when safe, with
scalar fallbacks and selectable scalar controls. Aggregates retain scalar f64
order. This is partial Arquero compatibility; see the guide for measurement
methodology, cache memory costs, and remaining APIs.
