<a id="組み込みコアライブラリ-walzcore"></a>

# Built-in core library `walz:core`

English | [日本語](README.ja.md)

Basic types are transparent aliases of existing base types; standard functions are ordinary Walz functions. Individual libraries add no runtime.
Import the needed functions and types from `walz:core` explicitly. Sources bundled with the compiler are statically linked, independent of caller location or installation paths.
Use `$option.ok-or` / `$result.map-err` for Option→Result and error conversion. See [evaluation order and constraints](../../docs/option-result.md).
For complete types, type constraints, and effects, see the [generated standard API list](../../docs/stdlib-api.md). Update it from public declarations with `just stdlib-docs` and check source agreement with `just check-stdlib-docs`.
The standard string type is `$string`. `bytes-view.walz` / `string-view.walz` provide [zero-copy views](../../docs/views.md), distinguishing shared subranges from explicit copies.
`$string-view.from-bytes-view` validates only the visible UTF-8 bytes and returns an optional shared string view. `$bytes-view.to-string` explicitly validates/materializes an owned string; invalid UTF-8 returns none before copying.
`bytes.walz` and `bytes-view.walz` also expose [fixed-width endian-specific readers](../../docs/byte-access.md). Mutable packed i8 arrays use explicit `@array.write-*` operations; bytes remain immutable.
`dict.walz` provides generic dictionaries with text keys; `list-builder.walz` constructs immutable lists. See [collections](../../docs/collections.md) for type constraints, mutation, and snapshot contracts.

```wat
(module
  (@use "walz:core" (type $string) (func $string.u64 $unsigned))
  (func $answer (export "answer") (result $string) (@effect reads writes allocates)
    (call $unsigned (i64.const 42))))
```

Each file's `@export` defines its API. Type names use lowercase kebab-case, such as `$string-view`; related functions use `type-name.operation-name`, such as `$string-view.new`. Select public names and bind them to arbitrary local names. See [WIT naming mappings](../../docs/naming.md).
Dependencies `json.walz` → `builder.walz` → `text.walz` → `bytes.walz` also use named imports. Each file loads once per compilation unit, sharing types and definitions.
Unselected names are not bound in callers. Generated Wasm retains only declarations reachable from actual ABI exports, tests, and laws.
See [static modules and DCE](../../docs/static-modules.md) for details. `include` remains ordinary text insertion.

| Import source | Function | Result and contract |
| --- | --- | --- |
| bytes.walz | `$bytes.equal(a,b)` | bytes content equality; i32 0/1 |
| bytes.walz | `$bytes.find-byte(b,start,byte)` | First position, or b's length if absent; also returns length for out-of-range start. byte is 0–255; other values never match |
| bytes.walz | `$bytes.find(b,needle,start)` | option<i32>; first match at or after start. Out-of-range gives none; empty needle matches valid start |
| bytes.walz | `$bytes.split-once(b,needle)` | option<pair<bytes,bytes>>; removes the first separator; none if absent |
| bytes.walz | `$bytes.parse-u64(b,start,length,limit)` | Reads the entire range as ASCII decimal, with unsigned-i64 limit. Empty input, invalid ranges/characters, and overflow give none |
| bytes.walz | `$bytes.concat(a,b)` | Concatenated bytes |
| text.walz | `$string.find(s,needle,start)` | option<i32>; UTF-8 byte position. none when start is inside a character or out of range |
| text.walz | `$string.split-once(s,needle)` | option<pair<text,text>>; preserves empty fields before/after and does not split remaining separators |
| text.walz | `$string.parse-u64(s)` / `$string.parse-s64(s)` | option<i64>; full u64/s64 ranges. Only signed parsing permits leading `-` |
| text.walz | `$string.concat(a,b)` | Concatenated text |
| text.walz | `$string.u64(n)` / `$string.s64(n)` | Unsigned/signed decimal text |
| json-value.walz | `$json.from-string(s)` / `$json.stringify(value)` | Constructs a JSON string value / generates JSON text |
| builder.walz | `$string-builder.new()` | Opaque mutable GC type `(ref $string-builder)` |
| builder.walz | `$string-builder.text(b,s)` | Appends text unchanged; returns unit. Combine with `$json.stringify` for JSON |
| builder.walz | `$string-builder.u64(b,n)` / `$string-builder.s64(b,n)` | Appends decimal text; returns unit |
| builder.walz | `$string-builder.finish(b)` | Returns joined immutable text and resets to empty; repeated finish returns empty text |
| json.walz | `$json.object-builder.new()` | Opaque mutable GC type `(ref $json-object)`; empty object |
| json.walz | `$json.object-builder.string(o,key,value)` / `$json.object-builder.u64(o,key,n)` / `$json.object-builder.s64(o,key,n)` | Adds a field of the matching type, escaping the key too; returns unit |
| json.walz | `$json.object-builder.boolean(o,key,n)` | 0 means false, other values true; returns unit |
| json.walz | `$json.object-builder.object(o,key,child)` | Finishes child and adds its JSON object as a value |
| json.walz | `$json.object-builder.finish(o)` | Returns JSON-object text and resets to empty; repeated finish returns `{}` |

Search, splitting, and integer parsing have semantic effect `pure`. Physical GC allocation to store values may still occur.

`checked-integer.walz` provides `$i32.checked-add-u`, `$i32.checked-mul-u`, and
`$i32.checked-align-up-u`, with matching `$i64.*` functions. They interpret unsigned
bit patterns and return Option; overflow and invalid alignment give none.
Alignment must be a nonzero power of two. Buffer/application limits are separate.
See [checked integers](../../docs/checked-integers.md) and the
[layout example](../../examples/checked-layout.walz).
Positions and lengths are treated as unsigned i32 bit patterns. `-1` is the maximum value and an invalid parsing range.
No string normalization or Unicode character/grapheme-count operations are performed. Splitting on an empty separator returns `(empty, original value)`.
bytes splitting accepts arbitrary byte sequences; text splitting preserves valid UTF-8 boundaries.
find-byte and integer parsing are linear in scan length. Substring find uses simple comparisons, with worst-case input length × needle length; split additionally copies the values before and after the separator.

Integer parsing permits leading zeros and rejects whitespace, `+`, separators, and non-ASCII digits. Signed `-0` is 0.
Limits are checked before multiplication, preventing wrapped values from succeeding.
Wasm i64 has no signedness distinction, so successful u64 values are also i64 bit patterns. At WIT s64 / JS BigInt boundaries, the highest bit appears as a sign bit.

Builders and JSON objects are shareable mutable GC objects; aliases observe the same state. Construction, mutation, and finish have reads/writes/allocates.
Finished text is independent of later changes, and builders retain no references to previous fragments.
Empty appends are ignored. A list proportional to fragment count and a join proportional to output length avoid copying the entire accumulated text on every append.
u32 overflow of total length and GC-array allocation failure are Wasm traps.

JSON fields retain insertion order. Duplicate keys are emitted unchanged. No API embeds unchecked external JSON text.
`$json.object-builder.object` resets child to empty. Even if parent and child alias, it captures the prior contents as a finite value before adding the field, avoiding cycles.
In addition to objects, strings, integers, and booleans, checked numbers, null, and JSON values can be appended. JSON numbers are retained as text.

`just test-stdlib` checks execution in Wasmtime and Core / Component. Browser checks are included in `just test-e2e`.
`just stdlib-capture DIRECTORY` fixes generated artifacts for three applications and hashes of inputs, expectations, and sources; capture under another name after changes.
`just stdlib-compare BEFORE AFTER` alternately measures fixed Core＋JSPI artifacts, checking each call's output, input termination, and zero outstanding ABI allocations.

`json-value.walz` provides immutable JSON values, parsing, array/object construction and access, and stringify.
The object builder in `json.walz` can also write `null`, checked `number`, and arbitrary JSON `value`.
See [JSON values and parser](../../docs/json-values.md) for numeric precision, input limits, and APIs.
