# Checked size and offset arithmetic

Import the checked integer helpers from `walz:core`. They are ordinary Walz
functions using existing Wasm numeric types and Option values, with no new syntax
or host dependency.

```wat
(module
  (@use "walz:core" (func $i32.checked-add-u $checked-add))
  (func $main (export "main") (result i32) (@effect pure)
    (@match (call $checked-add 40 2)
      (@case none () -1)
      (@case some ($sum) $sum))))
```

This returns 42. Failure is `none`; a successful zero is `some(0)`.

| Function | Parameters | Result |
| --- | --- | --- |
| `$i32.checked-add-u` | i32 a, b | option<i32> |
| `$i32.checked-mul-u` | i32 a, b | option<i32> |
| `$i32.checked-align-up-u` | i32 value, alignment | option<i32> |
| `$i64.checked-add-u` | i64 a, b | option<i64> |
| `$i64.checked-mul-u` | i64 a, b | option<i64> |
| `$i64.checked-align-up-u` | i64 value, alignment | option<i64> |

Each function has `(@effect pure)`. Inputs have exactly the declared scalar type;
there is no implicit width conversion. Function aliases work with ordinary `@use`,
and unused helpers are removed after the whole library has been checked.

## Unsigned bit patterns and failure

The `-u` suffix means unsigned interpretation of the existing i32/i64 bit pattern.
The valid arithmetic domain is 0 through 2^32−1 or 2^64−1. It does not introduce a
new unsigned type or change the behavior of ordinary Wasm instructions.

- Addition returns `none` if the mathematical sum exceeds the width's maximum.
- Multiplication returns `none` if the mathematical product exceeds that maximum.
  A zero operand succeeds with zero; the implementation guards division by zero.
- Alignment rounds up to the least multiple of a nonzero power-of-two alignment.
  Zero, other alignments, or an unrepresentable rounded result return `none`.
  The high-bit power of two is valid. Already aligned values remain unchanged.

For example, i32 bit pattern −1 represents 2^32−1: adding zero gives `some(-1)`,
while adding one gives `none`. Aligning that value to 1 succeeds; aligning to 2
fails. Alignment 3 fails even when the value is already divisible by 3.
`(i32.add (i32.const -1) (i32.const 1))` still wraps to zero.

Arguments evaluate once, left to right, before entering the function. The second
argument is evaluated even when the first multiplication operand is zero, or
when an earlier argument already determines failure. Existing effects, exceptions,
ownership, and early-return checks apply without special treatment.

## Allocation sizes and application limits

Use checked multiplication for `count * stride`, checked addition for headers or
offsets, then checked alignment. Compare the result with the actual buffer or
application limit separately. Representation-width checks do not impose a memory
budget. [The layout example](../examples/checked-layout.walz) combines all three
operations with an explicit limit and uses ordinary early return on `none`.

The FlatBuffers builder checks the vector byte count once before preparation and
allocation, retaining its 64 MiB limit and trap-based builder error policy. The
element loop then reuses the checked count.

These functions use the existing Option representation, so small physical GC
allocations may occur. `pure` describes semantic effects and is not an allocation
or performance guarantee. Hoist size checks outside element loops when possible.

At the current public JS/WIT boundaries, i32/i64 retain their s32/s64 storage
mapping. High-bit unsigned values consequently appear as negative Number/BigInt
values. For portable JS calls, use `Number(BigInt.asIntN(32, value))` or
`BigInt.asIntN(64, value)`; interpret a successful result with `>>> 0` or
`BigInt.asUintN(64, result)` as appropriate. `undefined` represents Option none.

Run `just test-checked-integers` for type/effect/DCE checks, Wasmtime boundary cases
with optimization on/off, and unbounded BigInt comparisons on both portable JS
backends and both experimental native modes. Browser coverage is in
`runtime/e2e/checked-integers.spec.mjs`. The existing independent ownership gate
is retained; no new universal arithmetic proof is claimed.
