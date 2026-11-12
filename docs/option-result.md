# Option/Result Value Operations

`walz:core` provides `$option.ok-or` and `$result.map-err` as ordinary monomorphized functions. They add no control syntax or ownership rules.

```wat
(module
  (@use "walz:core" (type $option) (type $result)
    (func $option.ok-or) (func $result.map-err))
  (@closure $Widen (param $error i32) (result i64) (@effect pure)
    (i64.extend_i32_u $error))
  (func $main (export "main") (result i64) (@effect allocates)
    (@match
      (call (@inst $result.map-err i64 i32 i64 $Widen)
        (call (@inst $option.ok-or i64 i32) (@option.some i64 42) 7)
        (@closure.new $Widen))
      (@case ok ($value) $value)
      (@case err ($error) -1))))
```

- `ok-or<T,E>(option<T>, E)` converts some to ok and none to err. As with an ordinary call, the error argument is evaluated first even for some. This operation does not defer effects or allocation.
- `map-err<T,E,U,F>(result<T,E>, F)` calls the callback once only for err. It preserves the ok payload unchanged, including GC reference identity. F is an explicit closure type satisfying `(param E) (result U)`. Callback effects contribute to the instantiated function's effects.

T/E/U must be shareable, non-unit values. Use the existing explicit `@match` for owned/borrowed payloads or unit. There are no implicit moves, extended borrows, or numeric conversions. See the [standard API](stdlib-api.md) for full signatures and `just test-value-sugar` for checks of both JS backends.

## Expected String Types

Where a string type is explicit, `(@string.lit "hello")` can be written as `"hello"`. This includes function parameters/results, lets annotated with `(type $string)`, fields, and explicitly typed lists. Transparent aliases and instantiated generic arguments follow the same rule.

```wat
(module
  (@use "walz:core" (type $string))
  (func $id (param $s $string) (result $string) $s)
  (func $main (export "main") (result $string)
    (@let $text (type $string) "日本😀"
      (call $id $text))))
```

When an initializer's expected type is unknown, as in `(@let $x "hello" ...)`, provide a type annotation or `@string.lit`. Non-UTF-8 byte sequences are not strings. `@string.byte-len` returns string length in UTF-8 bytes. There is no `@string.len` alias or implicit conversion to a Unicode character count.

Match arms use `@case`. A final `(@default fallback)` can handle remaining shared payloads without binding them; owned/borrowed cases must be explicit. See [match rules](syntax.md#match-clauses).

The [value optimizer](value-optimization.md) can eliminate private shared
tuple/Option/Result carriers used only by matches/drops, including small inlined
helper returns. Payload evaluation still occurs once, even for `@default`.
Escaping/owned containers and public ABI representations remain unchanged.
