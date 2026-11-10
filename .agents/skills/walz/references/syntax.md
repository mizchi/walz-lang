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

Prefer expected-type constructors such as `(@result.ok 42)` when a declared Result type supplies both type arguments. Otherwise spell out all required type arguments; Result takes **both** success and error types.

```wat
(module
  (@use "walz:core" (type $result))
  (func $answer (result (@inst $result i64 i32)) (@effect pure)
    (@result.ok 42))
  (func $main (export "main") (result i64) (@effect pure)
    (@match (call $answer)
      (case ok ($n) $n)
      (case err ($code) 0))))
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
(@match choice (case none () 0) (case value ($n) $n))
(@closure $Add (@capture $base i64) (param $n i64) (result i64)
  (@effect pure) (+ $base $n))
(@closure.new $Add 40)
(@closure.call $Add closure 2)
```

Named construction requires all fields. GC references may alias mutable data. Captures are explicit; owned captures make a closure single-use. Packed arrays and arbitrary WAT instructions may require the validator path rather than typed-core analysis.

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
