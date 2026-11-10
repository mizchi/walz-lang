<a id="v128-と-simd"></a>

# v128 and SIMD

`v128` is a primitive using Wasm's 128-bit value type directly; no import is needed. Names such as `i32x4` are not separate types, but describe how each instruction interprets the same bits. Walz adds no dedicated SIMD syntax and uses standard WAT instruction names.

```wat
(module
  (func $main (export "main") (result i32) (@effect pure)
    (@let $v v128 (v128.const i32x4 40 0 0 0)
      (i32x4.extract_lane 0
        (i32x4.add (local.get $v) (i32x4.splat (i32.const 2)))))))
```

`just run examples/simd.walz` returns 42. Instructions are checked in typed IR and emitted directly as corresponding Wasm SIMD instructions. SIMD operations add no runtime or heap allocations.

<a id="命令と型"></a>

## Instructions and types

| Operation | Examples and type conventions |
| --- | --- |
| Constants | `v128.const i8x16 …` / `i16x8 …` / `i32x4 …` / `i64x2 …` / `f32x4 …` / `f64x2 …`, with 16/8/4/2/4/2 lane values respectively |
| Splat | `(i32x4.splat i32-expr)` → `v128`; `i8x16` / `i16x8` also take `i32` |
| Extract | `(i32x4.extract_lane 0 vector)` → `i32`; specify `_s` / `_u` for `i8x16` / `i16x8` |
| Replace | `(i64x2.replace_lane 1 vector i64-expr)` → `v128` |
| Arithmetic, comparison, conversion | `i32x4.add`, `f32x4.mul`, `i16x8.eq`, `i32x4.trunc_sat_f32x4_s`, etc.; the type remains `v128` after shape conversions |
| Bit operations | `v128.and` / `or` / `xor` / `not`, and three-argument `v128.bitselect` |
| Shifts | `(i64x2.shl vector i32-expr)`; shift counts are `i32` for every shape |
| Reductions | `v128.any_true`, integer-lane `all_true` / `bitmask` → `i32` |
| Rearrangement | `i8x16.shuffle` takes 16 fixed indices from 0 to 31 and two `v128` values; `i8x16.swizzle` takes a dynamic index vector |

Lane numbers and shuffle indices are instruction immediates rather than expressions. Checks cover ranges, constant shapes/lane counts, argument counts, and types. Supported instructions come from the regular SIMD list in the pinned `wasmparser`; signatures and immediates are checked against the Wasm validator.

Integer wrapping, saturating arithmetic, comparison bitmasks, floating point, and NaNs follow [Wasm vector instructions](https://webassembly.github.io/spec/core/valid/instructions.html#vector-instructions). Walz adds no independent arithmetic rules.

<a id="gcジェネリクス解析"></a>

## GC, generics, and analysis

`v128` is shareable and satisfies `shared` / `nonunit` constraints. It can appear in function arguments/results, `@let`, GC struct/array fields, explicit closure captures, and type arguments such as `list` / `option`. Vectors themselves have neither reference identity nor ownership moves. Mutable-field reads/writes and container allocation follow existing effect rules.

Supported non-memory SIMD instructions themselves are pure and do not trap. Operands evaluate once each from left to right, retaining effects such as calls, reads/writes, exceptions, and suspension. `@effect pure` cannot hide operand effects.

Type, effect, ownership, and access-target analysis remain intact, but no vector range inference or SIMD algebraic simplification is added. The current Lean `@law` model likewise does not support SIMD and rejects it as an out-of-scope expression.

<a id="abi-と対応範囲"></a>

## ABI and supported scope

WIT / Component value types and the [WebAssembly JavaScript API](https://webassembly.github.io/spec/js-api/index.html) have no representation for passing `v128` directly. WIT generation, Component, and Core＋JSPI import/export boundaries reject direct `v128` and nested forms such as `list<v128>` / `future<v128>`. Use wrappers explicitly converting to scalars, scalar lists, or bytes.

`v128` is supported in internal Wasm functions, GC storage, and Future/Stream payloads. Internally constructed values can be retained across suspension, extracted to scalars, and returned. Standalone Core Wasm generation can emit Wasm `v128` signatures.

The typed core covers regular SIMD **non-memory instructions**. Linear-memory `v128.load` / `store` / load/store lane and Relaxed SIMD use the existing standard WAT path. Analysis on that path is `opaque` and does not guarantee typed-core effect/ownership contracts. Mixing unsupported instructions cannot bypass contract checks.

See the [minimal example](../examples/simd.walz) and [ABI/async example](../tests/component-fixtures/simd.walz). Rerun checks with `just test-simd` and browser checks with `pnpm exec playwright test runtime/e2e/simd.spec.mjs`.

For bulk-operation implementations and comparisons against JS and jsimd, see [SIMD kernels and measurements](simd-kernels.md).
