# Zero-Copy Views of Bytes, Strings, and Arrays

`$bytes-view` and `$string-view` are opaque standard-library GC types sharing part of immutable data.
Import needed types/functions explicitly from `walz:core`. No new syntax or runtime registry is added.
The standard string type is `$string`. Its `$` prefix makes it distinct from the WAT keyword `string`.
The underlying type/instructions use existing `@string`/`@string.*`; contents are strict UTF-8.

```wat
(module
  (@use "walz:core"
    (type $string)
    (func $string-view.new) (func $string-view.slice)
    (func $string-view.to-string))
  (func $crab (export "crab") (param $source $string)
    (result $string) (@effect allocates)
    (@let (($whole (call $string-view.new $source))
           ($part (call $string-view.slice $whole 1 7))
           ($crab (call $string-view.slice $part 3 4)))
      ;; "x日🦀z" なら "🦀"。コピーはここで行う。
      (call $string-view.to-string $crab))))
```

See [examples/views.walz](../examples/views.walz) for reading through views.
Exact signatures are in the [standard API listing](stdlib-api.md).

| Function | Operation | Complexity |
| --- | --- | --- |
| `$bytes-view.new(bytes)` | Creates a view of the whole value | O(1) |
| `$bytes-view.len(view)` | Byte length | O(1) |
| `$bytes-view.get(view,index)` | Reads a view byte as i32 from 0–255 | O(1) |
| `$bytes-view.read-i32-le(view,offset)` and other fixed-width readers | Reads an endian-specific value within the visible range | O(1) |
| `$bytes-view.slice(view,start,length)` | Shares a range relative to the view's start | O(1) |
| `$bytes-view.to-bytes(view)` | Copies the visible range into independent bytes | O(n) |
| `$bytes-view.is-utf8(view)` | Validates only the visible bytes without copying | O(n) |
| `$bytes-view.to-string(view)` | Validates and copies the visible range; returns option<string> | O(n) |
| `$string-view.new(string)` | Creates a view of the whole string | O(1) |
| `$string-view.from-bytes-view(view)` | Validates bytes and returns option<string-view> sharing the same storage | O(n) |
| `$string-view.byte-len(view)` | UTF-8 byte length | O(1) |
| `$string-view.bytes(view)` | Returns a bytes view of the same range | O(1) |
| `$string-view.slice(view,start,length)` | Checks UTF-8 boundaries and creates a subview | O(1) |
| `$string-view.to-string(view)` | Copies the visible range into an independent string | O(n) |

`new`, `slice`, and `string-view.from-bytes-view` have `allocates` because they allocate small GC objects.
The other operations have semantic effect `pure`. `pure` does not mean trap-free or free of physical GC allocation.
"Zero-copy" means view creation, reslicing, and reads do not copy the data itself.

## Bounds and UTF-8

Indices and lengths are unsigned i32 bit patterns, as with existing bytes operations; ranges use `start` and `length`.
Checks evaluate `start <= view.length` followed by `length <= view.length - start`, avoiding addition overflow.
Out-of-range `slice`/`get` causes a Wasm trap, without clamping or implicit rounding.
Reading outside the view is rejected even if still within the original buffer.
The same logical bounds apply to [fixed-width reads](byte-access.md): the complete 2/4/8-byte value must fit; unaligned offsets are allowed.

The [range optimizer](range-optimization.md) can carry validated immutable ranges
through small inlined helpers and per-iteration unsigned guards. Proven redundant
logical/reader guards can be removed; unknown ranges retain checks, and Wasm GC
array accesses remain bounds-checked. No unchecked view API is added.

String views also require start and end positions to be UTF-8 character boundaries.
For example, the whole `"é"` is `(start=0,length=2)`. `(0,1)` and `(1,0)` are rejected because they split a character.
`(2,0)` is a valid empty view. Character/grapheme counts and normalization are outside the scope.
A bytes view obtained from a string view can be split further at arbitrary byte ranges.

## Sharing and Reclamation

A bytes view holds a reference to the original bytes, a start, and a length.
A string view holds a bytes view satisfying UTF-8 invariants.
Reslicing refers directly to the original data rather than retaining a chain of parent views.
GC tracks references, so data stays valid after the caller returns and while waiting for a future.
Mutable views, borrow lifetimes, and explicit release are not added.

Even a small view retains the entire original buffer.
Use `to-bytes`/`to-string` to detach a portion for long-term storage. Whole-value views copy their contents too.
Once a view becomes unreachable, its reference to the original data is lost; copied values can survive independently.

Views are guest GC references and cannot be exported directly as WIT value types.
At Core+JSPI/Component public boundaries, convert explicitly to bytes/string and pass the result to existing ABI adapters.
Zero-copy transfer across the ABI is outside this API's contract.

## Validate a byte range as a string view

An arbitrary bytes view can become a string view after strict UTF-8 validation.
The conversion returns `none` on invalid UTF-8 and `some(view)` on success. Only
the visible range is validated; invalid prefix/suffix bytes in the backing buffer
are ignored. No byte array or owned string is allocated by this conversion.
The Option and successful view are small GC objects.

```wat
(module
  (@use "walz:core"
    (func $bytes-view.new) (func $bytes-view.slice)
    (func $string-view.from-bytes-view) (func $string-view.byte-len))
  (func $main (export "main") (result i32) (@effect allocates)
    (@let $part
      (call $bytes-view.slice (call $bytes-view.new (@bytes.new 255 240 159 166 128 255)) 1 4)
      (@match (call $string-view.from-bytes-view $part)
        (@case none () -1)
        (@case some ($text) (call $string-view.byte-len $text))))))
```

This returns 4: the four middle bytes encode `🦀`, although the owner is not valid
UTF-8 as a whole. Empty ranges are valid independently of surrounding bytes.
Truncated sequences, overlong encodings, surrogates, and code points above
U+10FFFF are invalid. NUL and BOM are valid characters; no BOM is stripped.
Subsequent string-view slices still require character boundaries.

`$bytes-view.is-utf8` performs the same scan and returns i32 0/1 without allocating
a view. `$bytes.is-utf8` validates a complete bytes value. The underlying primitive
is `(@bytes.is-utf8 bytes [start length])`. Indices are unsigned; malformed ranges
trap before any scan, while well-bounded invalid UTF-8 returns 0.

For an owned string, use `$bytes-view.to-string` or
`(@string.from-bytes bytes start length)`. Bounds and UTF-8 are checked before a
single visible-range GC byte-array copy. Invalid UTF-8 returns `none` without copying the
payload. The result does not retain the backing buffer; whole-range calls also
copy. The existing one-argument `@string.from-bytes` retains its sharing behavior.
`$string-view.to-string` uses the range operation and currently revalidates at
materialization, without an intermediate bytes wrapper/copy.
Public ABI transfers and native JS-string conversion retain their existing copies.

Validation views retain their original buffer just like other views. A small
validated string view can therefore keep a large binary message alive. Materialize
the visible text when that retention is undesirable. Validation adds no host
capability, lifetime syntax, or unchecked string constructor. Core+JSPI,
Component, and both experimental JS ABIs preserve the same strict UTF-8/BOM rules.
The validator and its lowering remain compiler TCB, without a new semantic proof.

Run `just test-utf8-views` for contract/GC tests and strict-decoder comparisons.
`just test-views` also covers validated views across suspension, rejection, and
cancellation. Browser checks are in `runtime/e2e/utf8-views.spec.mjs` and
`runtime/js-native-e2e/utf8-views.spec.mjs`.

<a id="任意のgc配列を共有するarray-view"></a>

## Array Views Sharing Arbitrary GC Arrays

`$array-view` is a read-only generic view with explicit element type `$T` and original named array type `$A`.
It uses existing generics and explicitly captured closures without new language syntax or array typeclasses.

```wat
(module
  (@use "walz:core"
    (func $array-view.new) (func $array-view.slice) (func $array-view.get))
  (type $Numbers (array (mut i64)))
  (func $main (export "main") (result i64) (@effect reads writes allocates)
    (@let (($source (array.new_fixed $Numbers 4 99 40 0 99))
           ($whole (call (@inst $array-view.new i64 $Numbers) $source))
           ($part (call (@inst $array-view.slice i64 $Numbers) $whole 1 2)))
      (block (result i64)
        (array.set $Numbers $source 2 2)
        (+ (call (@inst $array-view.get i64 $Numbers) $part 0)
           (call (@inst $array-view.get i64 $Numbers) $part 1))))))
```

This main returns 42. Existing views observe mutations of the original array.
The API adds neither writes through views nor an immutable-snapshot guarantee.

The type is `(ref (@inst $array-view T $A))`. `T` must be shared and nonunit; `$A` must be a shared named GC array type
whose element type matches `T`. Both mutable and immutable arrays are supported.
The target is the typed core's unpacked arrays, with elements such as numbers, v128, shared GC references, and strings.
Array kind and element type are checked when new is instantiated, even before a getter is called.

Call every function with `(call (@inst $array-view.<operation> T $A) ...)`.

| Operation | Parameters → result | Effect | Complexity |
| --- | --- | --- | --- |
| `new` | Original array → view | allocates | O(1) |
| `len` | view → i32 | pure | O(1) |
| `get` | view, index → T | reads | O(1) |
| `slice` | view, start, length → view | allocates | O(1) |
| `to-list` | view → `(@inst $list T)` | reads writes allocates | O(n) |

Ranges count elements; indices are relative to the view's start.
Unsigned i32 bounds checks match bytes-view; out-of-range, negative-bit-pattern, and overflowing ranges trap.
A zero-length slice at the end is valid. Get on an empty view traps; to-list returns an empty list.

New allocates a small reader capturing the original array and a view; slice allocates only a view sharing that reader.
There is no retained parent-view chain or copy of element storage. GC reaches the original array through the view, keeping it alive during suspension.
Unlike bytes/string views, get conservatively has reads because it may read a mutable array.

To-list reads the visible range into a working array and creates an immutable list of that range. A nonempty range allocates both the working array and list storage.
The list retains no references to the original array or reader. Reference-type elements themselves are shared, so this is not a deep copy of their graphs.
If no other references exist, including from elements, the original array can be reclaimed after discarding the view.
At WIT/JS ABI boundaries, explicitly convert with to-list only for publicly supported element types.
v128 and raw GC references remain unsupported by this ABI even as list elements. The GC-reference view itself also stays within the guest.

## Tests and References

`just test-views` checks bounds, Unicode, opaque/effect checks, Core/Component ABIs, and GC retention while waiting.
Array-view tests also cover mutable-array sharing, types/bounds, non-defaultable references and SIMD elements, empty copies, and out-of-bounds traps.
Tests check that reachable paths in generated IR contain no data copies; a test-only GC observer also checks
storage sharing, parent-view reclamation, and reclamation of original buffers after copying.
Chromium tests are included in `just test-e2e`.

Separating references and ranges was inspired by MoonBit's [BytesView](https://github.com/moonbitlang/core/blob/main/builtin/bytesview.mbt)
and [StringView](https://github.com/moonbitlang/core/blob/main/builtin/stringview.mbt).
MoonBit StringView uses UTF-16 code units; Walz string views check boundaries in UTF-8 bytes.
