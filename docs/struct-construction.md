# Named Struct Construction

`@struct.new` constructs an existing GC struct by field name. Every field in the type must be named.
Standard WAT positional `struct.new` remains available.

```wat
(type $User
  (struct (field $name $string) (field $age i32) (field $email $string)))

(@struct.new $User
  ($name (@string.lit "Alice"))
  ($age (i32.const 30))
  ($email (@string.lit "alice@example.com")))
```

Import `$string` as usual with `(@use "walz:core" (type $string))`.
Fields may appear in any order. Duplicate, unknown, and missing fields are static errors; nullable or mutable fields cannot be omitted either.
Labels do not bind variables. Initializers read existing locals with `$foo` or `(local.get $foo)`.

## Two Independent Sugars

Field groups can use `{}`. `:name` labels the static `$name` field.

```wat
(@struct.new $User {
  :name (@string.lit "Alice")
  :age (i32.const 30)
  :email (@string.lit "alice@example.com")
})
```

Directly inside a named initializer, literals whose types are determined by the field type can use shorthand. `{}` is optional.

```wat
(@struct.new $User
  ($name "Alice")
  ($age 30)
  ($email "alice@example.com"))
```

There is also a shorthand including the type name. `@$User::` selects `$User`, and `@$model.User::` selects `$model.User`.
`@` marks extension syntax; `$User` is an existing type identifier. `@User::` without `$` is rejected.

```wat
(@$User::{:name "Alice" :age 30 :email "alice@example.com"})
```

Bare numbers are checked as `i32.const`/`i64.const`/`f32.const`/`f64.const` of the resolved field type.
This is local expected typing based on field declarations. For example, `:age 30` in an i32 field receives the same constant instruction as `(i32.const 30)`; an i64 field receives `i64.const`.
Integer width is not chosen from literal magnitude, and later uses are not searched to find types. Existing declarations, instruction signatures, and explicit type applications determine types.
Passing an explicit `(i32.const 30)` to an i64 field is a type error, without implicit widening.
Ranges, signs, hexadecimal notation, floating-point values, and NaN/Inf follow WAT constant rules for the relevant type. For example, `4294967295` in an i32 field is an i32 with every bit set.
Bare strings use the same UTF-8 rules as `@string.lit` for `$string`. String escaping also follows WAT rules.
No default numeric widths, implicit conversions, automatic newtype wrapping, or boolean/null aliases are added.
Use existing explicit expressions for bytes, references, v128, newtypes, Option/Result, and similar values.
String shorthand is limited to direct named-construction initializers. Numeric and local-reference shorthand also works in [general expression positions with known expected types](core-expressions.md#ローカル参照と数値の省略形).

`{}` is used within a construction expression that specifies a type. Untyped maps, spread, commas, and dynamic keys are not added.
Values can be arbitrary existing expressions or other explicitly typed named-construction expressions. Construct an empty struct with `(@$Empty::{})`.

## Type Arguments and Visibility

This uses existing name resolution, explicit type application, and monomorphization.

```wat
(type $Box (@type.params $T) (@where (shared $T) (nonunit $T))
  (struct (field $value $T)))
(@struct.new (@inst $Box i64) {:value 42})
```

For type arguments, put `(@inst ...)` in the type operand of `@struct.new`. No type-argument inference is added.
Aliases of types imported through @use resolve under the same rules. Opaque types cannot be constructed externally.
Use existing positional construction for types containing anonymous fields.

## Evaluation Order and Static Analysis

Values evaluate **in source order, once per reached expression**, and are saved in hygienic temporaries.
Finally, the temporaries are arranged in type-definition order and passed to existing `struct.new`.
An intervening throw, trap, return, or branch skips later initializers and the final struct allocation.
Previously evaluated values are retained across Future awaits too. Reordering by field name or type-definition order never changes effect order.

Expansion uses existing typed-IR `Let`, `Local`, and `StructNew`, passing type, opaque, effect, ownership, and memory-region checks.
Name association happens at compile time without runtime keywords or maps. Struct identity, allocates, and mutable-field get/set retain Wasm GC rules.
The prohibition on storing owned values in shared GC fields remains. Initializers with ownership operations or exceptions use existing ownership certificates.
Unreachable later expressions remain subject to static checking.

See [struct-literals.walz](../examples/struct-literals.walz) for an executable example. `just run examples/struct-literals.walz` returns 42.
`just test-structs` checks types, execution, and both JS ABIs; `just test-e2e` checks Chromium.
The reading comparison before introduction is saved in the [evaluation record](../evals/reports/20261106-struct-literals-v2/REPORT.md).
A [current-syntax check](../evals/reports/20261106-struct-literals-implementation-v2/README.md) compiles and runs the same six patterns in four notations. Introduction-time records using the old `@Type::` spelling remain frozen with their original inputs/results.
