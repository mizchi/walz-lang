# Walz compact syntax

## Boundaries

Walz is a WAT superset. Standard WAT is checked by a Wasm validator. The high-level typed core additionally checks types, effects, ownership, and match coverage. An opaque WAT path does not carry the same detailed guarantees.

- WAT: `module`, `func`, `param`, `result`, `call`, `if`, `block`, numeric/GC/memory instructions.
- Walz: `@let`, `@loop`, `@for`, `@match`, `@try`, `@closure`, `@effect`, `@use`, `@export`.
- Operators: `+ - * / = != < <= > >= && || & | ^ << >> >>>`. They have two operands; numeric widths must agree. Logical operations use i32 and short-circuit.
- `$x` means `local.get`; numbers with a known expected scalar type mean the corresponding `.const`. Strings with expected `$string` type mean `@string.lit`.
- Function bodies contain one expression. Use a `block` for several operations.

## Functions and bindings

```wat
(module
  (func $add (param $a i64) (param $b i64) (result i64)
    (@effect pure) (+ $a $b))
  (func $main (export "main") (result i64) (@effect pure)
    (@let (($left (type i64) 40) ($right (type i64) 2))
      (call $add $left $right))))
```

```text
(@let $x [(type T)] initializer body)
(@let (($x [(type T)] initializer) ...) body)
(if (result R) condition (then yes) (else no))
(block $done (result R) ... (br $done value) ...)
(return value)
```

Grouped bindings are initialized once, left to right. No `local.set` on a let binding. Bare `(@let $n 42 ...)` has no integer width and fails.

## Explicit core imports and values

```text
(@use "walz:core" (type $string) (type $bytes) (type $list)
  (type $option) (type $result) (type $tuple) (type $future))
(@inst $result i64 i32)
(@inst $option i64)
(@inst $list i64)
(@result.ok i64 i32 42)
(@result.err i64 i32 1)
(@option.some i64 42)
(@option.none i64)
(@tuple.new i64 i32 40 2)
(@list.new i64 20 22)
```

Type and function applications also accept `$name<T, U>`, e.g.
`$result<$tuple<i32, i32>, i32>`, `(ref $Box<i32>)`, and
`(call $identity<i64> 42)`. Keep `<` adjacent to the name; arguments allow nesting,
comments/newlines and a trailing comma. Exact declared/imported WAT names win;
use `@inst` to explicitly apply a generic when a literal name collides. This
changes no inference, constraints, ownership, or runtime behavior.

Prefer expected-type constructors such as `(@result.ok 42)` when a declared Result type supplies both type arguments. Otherwise spell out all required type arguments; Result takes **both** success and error types.

```wat
(module
  (@use "walz:core" (type $result))
  (func $answer (result (@inst $result i64 i32)) (@effect pure)
    (@result.ok 42))
  (func $main (export "main") (result i64) (@effect pure)
    (@match (call $answer)
      (@case ok ($n) $n)
      (@case err ($code) 0))))
```

## GC data and closure captures

```text
(type $Counter (struct (field $value (mut i64))))
(@$Counter::{:value 40})
(struct.get $Counter $value counter)
(@struct.update $Counter counter ($value $old (+ $old 2)))
(@let {:value $n} counter body)
(type $Values (array (mut i64)))
(array.new_fixed $Values 2 first second)
(type $Choice (@variant (none) (value i64)))
(@variant.new $Choice value 42)
(@match choice (@case none () 0) (@case value ($n) $n))
(@closure $Add (@capture $base i64) (param $n i64) (result i64)
  (@effect pure) (+ $base $n))
(@closure.new $Add 40)
(@closure.call $Add closure 2)
```

Named construction requires all fields. GC references may alias mutable data. Captures are explicit; owned captures make a closure single-use. Packed i8/i16 arrays use i32 values and `array.get_s`/`array.get_u`; numeric `array.copy` is checked, including effects and opaque storage. `@bytes.from-array $A array [start length]` takes an immutable snapshot; `@bytes.copy-to-array $A dst at bytes start length` bulk-copies into mutable i8 storage. Arbitrary WAT instructions may still require the validator path.

Fixed-width reads are `(@bytes.read-i32-le bytes offset)` or imported `$bytes.read-i32-le` / `$bytes-view.read-i32-le` functions. Formats are i16/u16/i32/i64/f32/f64, with explicit le/be. i16 sign-extends to i32; u16 zero-extends. `(@array.write-i32-le $A array offset value)` writes only to named mutable i8 arrays (formats i16/i32/i64/f32/f64 × le/be, writes effect). Unsigned byte offsets may be unaligned; the full range must fit, relative to the visible range for views. Operands evaluate once left to right; all destination bounds are checked before mutation. Bytes stay immutable.

`$string-view.from-bytes-view(view)` returns option<string-view> after validating just the visible UTF-8 range, sharing its immutable backing bytes. It has allocates for a small view header, with no payload copy. `$bytes-view.is-utf8(view)` and `(@bytes.is-utf8 bytes [start length])` return i32 0/1. Invalid ranges trap; bounded invalid UTF-8 returns 0/none. `$bytes-view.to-string(view)` or `(@string.from-bytes bytes start length)` validates before copying the range into an owned string. BOM and NUL are preserved. Small views retain entire buffers; materialize to detach. String-view slices still require character boundaries.

Import `$i32.checked-add-u`, `$i32.checked-mul-u`, `$i32.checked-align-up-u` or their `$i64.*` counterparts for unsigned size arithmetic. Two same-width operands produce Option of that width; overflow/invalid alignment gives none, zero success gives some(0). Alignment must be a nonzero power of two, including the high-bit pattern. Negative signed displays still represent unsigned bit patterns. Normal Wasm arithmetic keeps wrapping, and buffer/application limits are separate. These are pure ordinary functions; Option can allocate physically and all operands are evaluated before the call.

## Generics, state, ownership

```text
(func $add (@type.params $T) (@where (Add $T))
  (param $a $T) (param $b $T) (result $T) (@effect pure) (+ $a $b))
(call (@inst $add i64) 40 2)
(@loop $sum (bind $i i32 0) (bind $total i64 0) (result i64) body)
(@recur $sum next-i next-total)
(@for-each $item list-value unit-body)
(@future.await i64 (@move $future))
(@future.close i64 (@move $future))
(@stream.next i64 (@move $stream))
(@stream.close i64 (@move $stream))
```

Recur must be at the tail of the innermost Walz loop. All next-state expressions are evaluated before state changes. Effects include `reads writes allocates suspends host` and `(throws $Tag)`; declare upper bounds honestly. Future/Stream operations consume endpoints, and Stream next returns a tuple containing the replacement endpoint and an Option item.

## A collection fold

`$list.fold<T,S,F>` takes `(list, closure, initial-state)`. The closure takes
`(state, element)` and returns the next state. Constructing a closure requires
`allocates`, even when the closure's body is pure. Use an explicit scalar literal
for a generic seed if the call does not provide enough expected-type information.

```wat
(module
  (@use "walz:core" (type $list) (func $list.fold))
  (@closure $AddPositive (param $total i64) (param $value i64) (result i64)
    (@effect pure)
    (+ $total (if (result i64) (> $value 0) (then $value) (else 0))))
  (func $main (export "main") (result i64) (@effect allocates)
    (call (@inst $list.fold i64 i64 $AddPositive)
      (@list.new i64 40 -1 2) (@closure.new $AddPositive) (i64.const 0))))
```

## Source and host modules

```text
(@export (func $add) (type $Row))
(@use "./math.walz" (func $add $local-add))
(@use "walz:core" (func $string.u64))
(import "host" "read" (func $read (result i64)))
(export "main" (func $main))
```

Keep source libraries free of Wasm ABI exports and `@test`/`@bench` entry declarations. Put those in root/test/benchmark files. A host import also needs a trusted effect/capability policy for portable JS/Component output. Avoid `@include` when selecting library names is sufficient.

## Review notation

`walz print source.walz` defaults to canonical Walz; `--format watz` selects reversible `.watz` syntax; `walz print source.watz` restores S-expressions. `func`, `@export("name")`, typed `let`, selectors and parenthesized operators are surface sugar; `wat.*` and `walz.*` preserve other forms. The file is an implicit module; `module { ... }` is optional and the printer omits it. `let` is immutable and assignment/`const` are rejected. This is Walz input, not JS/TypeScript execution. Use ordinary check/build/run and preserve explicit types, effects, captures, and ownership. A final `(@default body)` covers remaining shared-payload variants without bindings; owners/borrows require explicit `@case`.

Direct Watz branches use `match ($value) { case ok($n) => $n, default => 0, }` and `if<i64> ($n > 0) { $n } else { 0 }`. `if<T>` preserves `(result T)`; no result annotation means unit. Explicit match headers are `match<$Variant, R>` or `match<$Variant>` with no result declaration. Branch braces contain an expression, without a return statement or trailing semicolon; use `wat.block` for sequencing. The ordinary type/effect/ownership checks and exhaustive-case rules still apply. Native labeled/stack-style forms retain structural spelling.

Watz match clauses use `case`/`default`; canonical Walz keeps `(@case ...)`/`(@default ...)`.
