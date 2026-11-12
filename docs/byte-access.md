# Fixed-width byte access

Read integers and floating-point bit patterns from immutable bytes, or write them
to a named mutable packed i8 GC array. The endian is always explicit: `le` or `be`.
Offsets are byte offsets, including for unaligned accesses.
For a single byte, use the existing `@bytes.get` and packed `array.set` operations.

```wat
(module
  (@use "walz:core" (type $bytes) (func $bytes.read-i32-le))
  (type $Storage (array (mut i8)))
  (func $main (export "main") (result i32) (@effect reads writes allocates)
    (@let $a (array.new $Storage 0 5)
      (block (result i32)
        (@array.write-i32-le $Storage $a 1 42)
        (call $bytes.read-i32-le (@bytes.from-array $Storage $a) 1)))))
```

This returns 42. The write changes the GC array; `@bytes.from-array` creates an
independent immutable snapshot. Bytes themselves have no write operation.

## Operations and types

For each format below, substitute `le` or `be` for `ENDIAN`.

| Format | Read result / write value | Storage width | Meaning |
| --- | --- | --- | --- |
| `i16` | `i32` | 2 bytes | Sign-extend on reads; store the low 16 bits on writes |
| `u16` | `i32` | 2 bytes | Zero-extend on reads; use the `i16` writer for either 16-bit bit pattern |
| `i32` | `i32` | 4 bytes | Preserve all 32 bits |
| `i64` | `i64` | 8 bytes | Preserve all 64 bits |
| `f32` | `f32` | 4 bytes | IEEE-754 bit reinterpretation |
| `f64` | `f64` | 8 bytes | IEEE-754 bit reinterpretation |

Reads have three equivalent entry points:

```text
(@bytes.read-i32-le bytes offset)
(call $bytes.read-i32-le bytes offset)
(call $bytes-view.read-i32-le view offset)
```

Import function names explicitly from `walz:core`. All twelve `$bytes.read-*`
and twelve `$bytes-view.read-*` functions have semantic effect `pure`, allocate no
payload storage, and may trap. View offsets are relative to the visible range;
accesses outside that range trap even when the backing bytes are long enough.

Writes use the explicit named storage type:

```text
(@array.write-i32-le $Storage array offset value)
```

There are ten writers: `i16`, `i32`, `i64`, `f32`, and `f64`, each in both endians.
The array operand must be a non-null reference to the specified `(array (mut i8))`
type. Other storage widths, immutable arrays, and implicit numeric conversions
are rejected. Writers return unit and have the `writes` effect. Opaque array
representations remain private, including in macro expansions.

## Bounds, evaluation, and floating point

Operands evaluate exactly once, left to right, before the access checks. A throw,
trap, or suspension in an operand follows the existing evaluation and cleanup
rules. Once operands finish, check the unsigned i32 offset against the byte length,
then check `width <= length - offset`. Negative offsets are large unsigned values.
An incomplete or overflowing range traps; unaligned complete ranges are accepted.
The full destination range is checked before the first byte is changed. This does
not make the write atomic across threads; these are ordinary agent-local GC arrays.

Integer operations use Wasm bit patterns, with no new unsigned numeric type.
Floating-point reads and writes reinterpret bits without arithmetic or conversion,
preserving signed zero and NaN payloads within Wasm. Passing a float through a JS
number boundary can change its NaN representation; use integer reinterpretation
when exact payload bits matter.

The compiler retains dedicated typed IR operations and emits only used helpers.
Current lowering uses checked GC array accesses and shifts; it requires neither
linear memory nor a host import and does not promise a single machine load/store.
Type, effect, ownership, visibility, and independent ownership-contract checks
remain in force. Instruction-level semantic preservation is part of the compiler
TCB, rather than a new Lean proof claim.

The [immutable-range optimizer](range-optimization.md) can omit redundant explicit
reader range guards after proving the complete range fits. The internal proven
reader still uses Wasm GC bounds-checked `array.get_u`; it has no source spelling.
Writers retain their complete-range check before mutation. Logical view bounds
also remain unless proved, even when the backing buffer is larger.

Run `just test-byte-access` for type/contract and DataView differential checks on
Core+JSPI and Component. Chromium checks are in `runtime/e2e/byte-access.spec.mjs`.
