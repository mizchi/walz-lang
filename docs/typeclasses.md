# Primitive Typeclasses and Symbolic Operators

Built-in `Add`, `Sub`, `Mul`, `Div`, `Eq`, `Ord`, `Logical`, `Bitwise`, and `Shift` provide fixed instances for concrete types.
Type checking resolves operators into existing Wasm instructions or if expressions without dictionaries, indirect calls, or runtime instance search.
Classes are compiler built-ins and need no imports. User-declared class/impl syntax is not provided at this stage.

| Class | Instances | Operator | Expansion |
| --- | --- | --- | --- |
| `Add` | i32 / i64 / f32 / f64 | `(+ a b)` | Corresponding `.add`; result matches operand type |
| `Sub` | i32 / i64 / f32 / f64 | `(- a b)` | Corresponding `.sub`; result matches operand type |
| `Mul` | i32 / i64 / f32 / f64 | `(* a b)` | Corresponding `.mul`; result matches operand type |
| `Div` | i32 / i64 / f32 / f64 | `(/ a b)` | Signed `.div_s` for integers, `.div` for floats; result matches input type |
| `Eq` | i32 / i64 | `(= a b)` / `(!= a b)` | `.eq` / `.ne`; result is i32 0/1 |
| `Ord` | i32 / i64 | `(< a b)` / `(<= a b)` / `(> a b)` / `(>= a b)` | Signed `.lt_s` / `.le_s` / `.gt_s` / `.ge_s`; result is i32 0/1 |
| `Logical` | i32 | `(&& a b)` / `(\|\| a b)` | Short-circuiting if; result is i32 0/1 |
| `Bitwise` | i32 / i64 | `(& a b)` / `(\| a b)` / `(^ a b)` | `.and` / `.or` / `.xor`; result matches input type |
| `Shift` | i32 / i64 | `(<< a n)` / `(>> a n)` / `(>>> a n)` | `.shl` / `.shr_s` / `.shr_u`; result matches input type |

Every operator takes two arguments of the same type. Fixed symbolic operators have no `@` prefix.
Integer addition/subtraction/multiplication wraps at the Wasm bit width. Integer division truncates toward zero and traps on division by zero or minimum-value/-1. Use explicit `i32.div_u`/`i64.div_u` for unsigned division.
Floating-point arithmetic retains Wasm rounding, NaN/Infinity, and related rules. Float division by zero does not trap, and operations are not reordered assuming associativity.
Floating-point types do not implement Eq/Ord because NaN prevents ordinary equivalence relations and total ordering.
Use existing explicit Wasm instructions for floating-point comparisons, unsigned-integer comparisons, and v128.

## Short-Circuit Logic and Bitwise Operations

Booleans use i32 as existing conditions do: 0 is false, nonzero is true. `&&`/`||` always return 0/1 rather than an unchanged operand.
`(&& a b)` evaluates b only when a is true; `(|| a b)` evaluates b only when a is false. The left operand evaluates once.

```wat
(module
  (func $guard (export "guard") (param $ready i32) (result i32)
    (|| $ready (i32.div_s 1 0)))
  (func $main (export "main") (result i32)
    (call $guard 7)))
```

This main returns 1 without executing the right-hand division. `(&& 2 4)` returns 1, while bitwise `(& 2 4)` returns 0.
`&`, `|`, and `^` evaluate both operands once each from left to right.

Shifts require the value and count to have the same integer type. `>>` sign-extends; `>>>` zero-fills.
As in Wasm, i32 uses only the low 5 count bits, and i64 uses the low 6. Thus i32 `(<< 1 32)` and i64 `(<< 1 64)` both return 1. Negative shift counts follow the same masking rule.
`>>>` does not change the result type. For example, `(>>> -1 0)` remains an i32/i64 with every bit set.

```wat
(module
  (func $masked (@type.params $T) (@where (Bitwise $T) (Shift $T))
    (param $x $T) (param $mask $T) (param $n $T) (result $T)
    (>>> (& $x $mask) $n))
  (func $main (export "main") (result i64)
    (call (@inst $masked i64) 84 127 1)))
```

A right operand that may be skipped by short-circuiting still undergoes type, effect, and ownership checking.
Consuming an outer owned value only on the right creates inconsistent post-branch ownership states and is an error. Values created and consumed inside the right operand are allowed.
Expansion uses existing If/Numeric forms, so throw, return, trap, and Future awaits follow the same paths as ordinary if.

## Determining Types

```wat
(module
  (func $four (result i32) (+ 1 3))
  (func $main (export "main") (result i64)
    (@let (($x (type i64) 40)
           ($y (+ 1 $x)))
      (+ $y 1))))
```

For `+ - * /`, bitwise operations, and shifts, whose results match their inputs, the input type comes from an expected result/parameter/let-annotation/field type or from an operand with a known type.
Once determined, both operands are checked at that type. No implicit width or integer/float conversions occur.
Regardless of which operand determines the type, reached operands evaluate once each in source order. Throw, return, waiting, and ownership rules are those of the original expressions.

Comparisons return i32, but that result type does not determine operand width.
If `$x` is i64, `(< $x 3)` is an i64 comparison. When both sides are bare numbers, make one explicit, such as `(< (i64.const 1) 3)`.
`(@let $x (+ 1 3) body)` is also an error without a type clue from operands or the expected type. There is no default numeric type.
`Logical` inputs are fixed to i32, so `(&& 1 3)` and `(@let $x (|| 0 7) body)` have determined types. i64, floats, references, and other types do not implicitly convert to booleans.

Transparent type aliases use their expanded types. Newtypes do not automatically inherit base-type instances.
Unwrap newtypes explicitly before operations, then wrap if needed. No structural Eq or similar operations for text, bytes, GC references, or owned values are added.

## Generic Bounds

```wat
(module
  (func $add (@type.params $T) (@where (Add $T))
    (param $a $T) (param $b $T) (result $T) (@effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64) (@effect pure)
    (call (@inst $add i64) 40 2)))
```

Within `@where`, class names from the table specify constraints such as `(Add $T)`, `(Mul $T)`, or `(Div $T)`; `(Add $T $U)` imposes the same constraint on both types.
This is shared by func/type/type.alias/closure. Targets must be type parameters of that declaration.
Every constraint must hold; checks also cover bodies that never invoke operators and applications from functions removed by DCE.
Unsupported types produce `E_GENERIC_BOUND` before substituted arguments can create invalid headers.

Type arguments continue to be explicit, such as `(@inst $add i64)`.
As with existing generics, each instantiation checks body types, effects, and ownership. There is no declaration-time proof of abstract bodies or automatic inference of required constraints.
Typeclasses indicate instance availability; they do not automatically prove algebraic laws for arbitrary types. Integer `@law` checks the existing resolved instructions as before.

Main diagnostics are `E_OPERATOR_TYPE` for undetermined types, `E_TYPE_MISMATCH` for mixed types, `E_CLASS_INSTANCE` for directly unsupported operations, and `E_GENERIC_BOUND` during instantiation.
`type-at` distinguishes symbolic-operator result types from operand types.

See [typeclasses.walz](../examples/typeclasses.walz) for an executable example. `just test-typeclasses` checks agreement with Wasm, monomorphization, diagnostics, and both JS ABIs; `just test-e2e` checks Chromium.
