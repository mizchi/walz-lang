# Immutable list<T>

`(@inst $list T)` is a built-in value type instantiated for each element type. There are no operations that mutate the container;
as with `$bytes`, its internal GC array cannot be referenced, compared, or modified.
Elements can be shareable numbers, value types, or GC references. Owned values and borrows cannot be stored.
Mutability of objects referenced by GC-reference elements follows the rules of those reference types.

| Syntax | Result/properties |
| --- | --- |
| `(@list.new T value...)` | Constructs a list, evaluating each value once from left to right; no arguments produces an empty list |
| `(@list.len T xs)` | Element count as i32 |
| `(@list.get T xs index)` | An element; indices are unsigned, and out-of-bounds access causes a Wasm trap |
| `(@list.slice T xs start length)` | Copies a range; out-of-bounds ranges trap; length 0 at the end is valid |
| `(@list.from-array T $Array source)` | Copies a GC array with the same element type |

Slice/from-array copies the container. GC-reference elements share their references.
Changing the original mutable array does not change element slots in the copied list.
For `list.new unit`, supply unit expressions such as `(nop)` for the desired element count.

```wat
(@list.get i64
  (@list.new i64 (i64.const 40) (i64.const 2))
  (i32.const 1))
```

`examples/list-batch.walz` contains map, which constructs a result in a mutable GC array then copies it into a list, and sum, which reads a list.
For generic map/fold, see [monomorphization](generics.md) and
`examples/generic-lists.walz`.

WIT uses `list<T>`; the Canonical ABI uses a pointer/length pair for the element region.
Element size/alignment determines layout, and nested regions are released recursively. Reads check length, multiplication,
alignment, and memory bounds. The existing ABI linear-memory limit is 16 MiB.
The engine manages Wasm GC array reachability; no custom GC runtime is added.

JS accepts arrays and, for numeric elements, the corresponding TypedArray.
Numeric list results are Int32Array/BigInt64Array/Float32Array/Float64Array; other results are Array.

Core+JSPI bulk-copies matching TypedArray inputs and numeric list outputs on little-endian systems.
`i32`, `i64`, `f32`, and `f64` use `Int32Array`, `BigInt64Array`, `Float32Array`, and `Float64Array`, respectively.
Intrinsic getters check the actual element type and length, preventing prototype modifications or Proxies from bypassing value checks.
Ordinary Arrays, views with overridden lengths, and big-endian systems use the existing per-element checking/conversion path.
ABI length limits, alignment checks, and bounds checks remain in place.

Results are copies independent of the ABI region, unaffected by input mutation, ABI release, or subsequent memory growth.
The bulk-copy path also preserves NaN payloads and negative-zero bit patterns between matching TypedArrays.
Same-element-type copying follows [ECMAScript TypedArray transfer](https://tc39.es/ecma262/multipage/indexed-collections.html#sec-settypedarrayfromtypedarray).
This improves transfer between JS and linear memory; conversion to GC lists and SIMD packing are still separate work.
This representation matches generated Component bindings. s64 values use BigInt.
WIT `list<u8>` continues to map to `$bytes`/Uint8Array.
Lists can be shareable Future/Stream payloads, but Futures/Streams cannot be stored in lists.

Run `just test-lists` to check Rust typing/ownership/escape analysis, Wasmtime/Node effect order,
round trips and reclamation after partial conversion failure in both JS backends, and Chromium boundary behavior.
The ownership contract language is unchanged; list arguments are checked as existing Seq forms.

On conversion failure or traps, endpoints registered with the guest are closed before reclaiming the ABI arena.
The failed-copy state in pinned jco 0.23.0 is also terminated, waking pending Streams with failure.
Failure tests check registry counts, iterator.return, and the absence of unhandled Promises.
