# Numbers, Result Chaining, Newtypes, and Early Exits

All of these are checked in the typed core. Standard WAT numeric instructions, `return`, `block`, `br`, and `br_if` have no `@` prefix; additional sugar and type operations do.

## Scalar Numeric Instructions

Direct support includes `i32`/`i64` rotations and sign extension, float/integer conversions, `f32`/`f64` `abs`, `ceil`, `floor`, `trunc`, `nearest`, `sqrt`, `min`, `max`, and `copysign`, `reinterpret`, and `demote`/`promote`. Instruction signatures and trap classification are centralized in `numeric_ops!` in `src/ir.rs`.

`(i32.trunc_f64_s (f64.sqrt (f64.const 1764)))` evaluates to 42. Conversions introduce no implicit rounding rules. `trunc_*` causes a Wasm trap for NaN or out-of-range values; `trunc_sat_*` saturates. `nearest` uses Wasm's round-to-even rule. The semantic effect `pure` does not mean trap-free. The scope of integer law proofs does not expand automatically either.

<a id="ローカル参照と数値の省略形"></a>

## Local References and Numeric Shorthand

`$foo` in an expression position means `(local.get $foo)`. It can refer to bindings from parameters, lets, loop state, cases, catches, and explicit closure captures. Field names create no bindings, and globals/functions are not referenced automatically. Owned values still require `(@move $foo)`.

```wat
(module
  (func $inc (param $foo i64) (result i64)
    (i64.add $foo 1))
  (func $main (export "main") (result i64)
    (@let (($start (type i64) 41)
           ($answer (call $inc $start)))
      $answer)))
```

Let type annotations can use `(type T)`. This works both for a single `(@let $x (type i32) 1 body)` and for multiple bindings; T can be an alias, reference type, or `(@inst ...)`. An unannotated `(@let $x $foo body)` uses the existing type of `$foo`.

A bare number expands to the corresponding `.const` in expression positions where the expected type is determined to be `i32`, `i64`, `f32`, or `f64`. Expected types come from annotations, function parameters/results, numeric/SIMD instruction operands, declared block/if/match results, struct/array elements, explicitly typed value construction, and similar contexts. Conditions and array indices are i32. `(i64.eq 1 2)` compares i64 constants and returns i32.

`(@let $x 1 body)` and `(drop 1)` are errors because they have no expected type. Width is not selected from the magnitude of a number, and types are not inferred backward from later uses. An omitted `(result)` still means unit; a bare numeric body does not cause result-type inference.
Ranges, hexadecimal notation, floating-point values, and NaN/Inf follow WAT constant rules. An explicit `(i32.const 1)` or i32 local is not implicitly converted to i64, and newtypes are not automatically wrapped. Use explicit `v128.const` for v128.

String shorthand is limited to [named struct initializers](struct-construction.md). WAT stack syntax and immediates outside the typed core are not covered by this shorthand. Input valid as standard WAT continues to compile unchanged.
Expansion uses the existing Local/Const representation, passing the same information to type/effect/ownership/region checking, optimization, and `type-at`. See [expression-sugar.walz](../examples/expression-sugar.walz) for an executable example and `just test-expression-sugar` for checks.

Primitive types also have [typeclass operators](typeclasses.md) such as `+`, `=`, and `<`. Once the type is known, these expand directly into the existing numeric instructions covered here.

## `@result.do`

```wat
(@result.do
  ($a (call $first))
  ($b (call $second (local.get $a)))
  (() (call $check (local.get $b)))
  (@result.ok (local.get $b)))
```

Provide one or more bindings followed by a final Result expression. Each binding is `($name Result-expression)`, or `(() Result-expression)` for a unit success payload. Evaluation proceeds once per expression from left to right; success names are available in subsequent bindings and the final expression. The first err skips the remaining expressions and becomes **the result of this expression**. It does not implicitly return from the function.

This expands into nested existing `@result.let` forms. Success types may differ at each stage, but every error type must be the same shareable type. Owned success payloads require explicit moves. The err path must not leave owned values unconsumed either.

## Scalar Newtypes

```wat
(type $UserId (@newtype i64))
(@newtype.wrap $UserId (i64.const 42))
(@newtype.unwrap $UserId id-expression)
```

The base is one of `i32`, `i64`, `f32`, or `f64`. Write `$UserId` in type positions. It is distinct from other newtypes with the same base and from the underlying numeric type; conversions are explicit. Unwrap before arithmetic and wrap the result if needed. Newtypes satisfy shared/nonunit and can be used in GC fields, enums, standard value types, and monomorphization type arguments.

They are erased after type, effect, ownership, and opaque checking. Wrap/unwrap creates no GC objects or runtime type tags. `types`/`type-at` retains nominal type names. Wrapping/unwrapping a type exported as `opaque` is available only within its declaring file.

At Core+JSPI and Component boundaries, newtypes pass as their underlying scalar, including in list or Future payloads. JavaScript/WIT values retain no type name or brand, so this does not guarantee range constraints on external input. Check ranges separately in public constructor functions. Newtypes based on GC references or v128 and implicit delegation of operations are not provided.

## Return and Labeled Branches

```wat
(block $done (result i32)
  (drop (br_if $done (i32.const 42) condition))
  (return (i32.const 7)))
```

The value of `return` must match the function result type; use `(return)` in a unit function. `br`/`br_if` names an enclosing block or loop, matching the block result or loop input state. `br_if` evaluates its value before its condition and branches if the condition is nonzero. Otherwise the value remains as the expression result. In the example above, the false path drops 42 and returns 7.

Typed-core results are unit or a single value. Standard WAT using numeric labels, multiple results, `br_table`, and similar features uses the existing WAT validation path. `@recur` remains restricted to tail positions in the innermost Walz loop. `@for` also expands into ordinary labels. No non-local exceptions or dedicated break/continue forms are added.

When return, a branch, or recur exits `@using`, values and conditions are evaluated first, then resources in exited scopes are closed from the inside out in reverse order. Leaving only an inner block does not close outer resources. The release flag is cleared before invoking a destructor, preventing a second call during exception unwinding. Borrows cannot be branch payloads.

## Ownership on Exception Paths

`@try` and throwing calls can be used while holding owned values. Checking requires matching ownership sets at every catch entry, ordinary join, and loop entry. Every exit path must complete moves, returns, destructuring, or close. Throws from handlers propagate outward.

Owned values in partially evaluated arguments and similar contexts are also tracked. In `(call $consume (@move $x) (call $may-throw))`, a later argument throwing would strand the earlier owned value; such programs are rejected. Bind throwing computations first, or handle failure before acquiring/moving the value.

`contracts` emits version 2 control-flow certificates for functions using the new control forms. Lean independently checks each node's available ownership slots and transitions. Existing version 1 contracts are also accepted. Lean's theorems cover each transition and ownership states along paths containing any number of iterations. Extraction from IR, effect inference, Wasm emission, borrow non-escape, and the host resource registry remain in the TCB.

Owned exception payloads, Future/Stream payloads containing owned values, partial moves, and general lifetimes are not added. Wasm traps are not caught; external resources also use host-registry shutdown cleanup.

`just test-core-expressions` checks numbers, Result, newtypes, control flow, exception ownership, and both ABIs; `just test-e2e` checks Chromium. `examples/resources/main.walz` demonstrates early exits and release, and `tests/component-fixtures/language-core.walz` combines these features.
