# Writing Walz programs

After [getting started](getting-started.md), work through functions, values, data structures, iteration, and modules.
Every `wat` block below is a complete module; examples with a `main` entry return 42.
Unless a filename is specified, save a block as `main.walz` and use `walz check main.walz` and `walz run main.walz`.
The file-module section combines a library with a separate entry file.
Use the [cheat sheet](../CHEATSHEET.md) for syntax lookup and the [feature list](features.md) for supported boundaries.

## Recognize WAT and Walz extensions

Expressions use `(operation arguments...)`. Identifiers such as `$name` and line comments starting with `;;` follow WAT.
`func`, `call`, `if`, and `struct.get` are Wasm instructions or declarations.
Walz extensions start with `@`, including `@let`, `@match`, and `@closure`.
Fixed operators such as `+`, and unambiguous local/value shorthand, are exceptions without a prefix.

```wat
(module
  (func $add (param $a i64) (param $b i64) (result i64)
    (@effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64) (@effect pure)
    (call $add 40 2)))
```

`$a` means `(local.get $a)`; `40` in an i64 position means `(i64.const 40)`.
Here `+` resolves to `i64.add`. Integers wrap at the Wasm bit width, and division retains Wasm's trap behavior.
Declare function parameters and results. Omitting the result means unit.

Standard WAT goes through the Wasm validator. Additional Walz type, effect, and ownership checks apply to the typed core;
these guarantees do not extend to arbitrary WAT. See [syntax boundaries](syntax.md).

## Immutable locals and expected types

`@let` binds a value and returns its body expression. Grouped bindings initialize once from left to right.

```wat
(module
  (func $main (export "main") (result i64) (@effect pure)
    (@let (($base (type i64) 40)
           ($delta (type i64) 2)
           ($answer (+ $base $delta)))
      $answer)))
```

Later initializers can read earlier bindings. Names are local to the body and cannot be reassigned.
A single binding can also use `(@let $x (type i64) 42 $x)`.
An initializer such as `(@let $x 42 ...)` lacks an integer width and is rejected.
Convert explicitly with instructions such as `i64.extend_i32_s`. See [bindings](standard.md) and [numeric inference](core-expressions.md).

## Conditional expressions

A condition is i32: zero is false, nonzero is true. Both branches must agree on their result type.

```wat
(module
  (func $choose (param $n i64) (result i64) (@effect pure)
    (if (result i64) (> $n 0)
      (then $n)
      (else 0)))
  (func $main (export "main") (result i64) (@effect pure)
    (call $choose 42)))
```

`>` performs a signed integer comparison. Use the corresponding Wasm instructions for floats and unsigned integers.
`&&` and `||` short-circuit and return i32 zero or one. See [operators](typeclasses.md).

## GC data and mutable state

Wasm GC manages structs, arrays, and closure environments. Mark mutable fields with `mut`.
Import every core type/function you use explicitly from `walz:core`.

```wat
(module
  (@use "walz:core" (type $string))
  (type $Counter
    (struct (field $label $string) (field $value (mut i64))))
  (func $main (export "main") (result i64) (@effect reads writes allocates)
    (@let $counter (@$Counter::{:label "accepted" :value 40})
      (block (result i64)
        (@struct.update $Counter $counter ($value $old (+ $old 2)))
        (@let {:value $n} $counter $n)))))
```

`@$Counter::{...}` constructs a named type with named fields. Every field is required; this is not an untyped map literal.
`@struct.update` reads the target and old value once, then writes the result. It is not an atomic update.
Aliases to a GC object observe its mutable state. An immutable let binding does not make the referenced object immutable.
See [named construction](struct-construction.md) and [field operations](high-level-forms.md).

## Enums and exhaustive matching

Declare alternatives with `@variant`; the checker validates payload types and complete case coverage.

```wat
(module
  (type $Decision (@variant (answer i64) (missing)))
  (func $main (export "main") (result i64) (@effect allocates)
    (@let $decision (@variant.new $Decision answer 42)
      (@match $decision
        (case answer ($n) $n)
        (case missing () 0)))))
```

An empty payload still needs `()`. Missing/duplicate cases and incorrect payload types or counts are errors.
Ordinary enums are GC references; Option/Result below are value types with hidden representations. See the [cheat sheet](../CHEATSHEET.md).

## Return absence and failure with Option/Result

Use `$option` for absence and `$result` for application failure.
The outer `(result ...)` declares a WAT return type; `(@inst $result T E)` is one value with success type T and failure type E.

```wat
(module
  (@use "walz:core" (type $result))
  (func $positive (param $n i64) (result (@inst $result i64 i32))
    (@effect pure)
    (if (result (@inst $result i64 i32)) (> $n 0)
      (then (@result.ok $n))
      (else (@result.err 1))))
  (func $main (export "main") (result i64) (@effect pure)
    (@let $value (call $positive 42)
      (@match $value
        (case ok ($n) $n)
        (case err ($code) 0)))))
```

Option has `none`/`some`; Result has `ok`/`err`.
Use `@result.do` to sequence successful results. Traps such as integer division by zero do not automatically become Result errors.
Wasm exceptions use tags and `@try`, separately from error values. See [value operations](option-result.md) and [Result sequencing](core-expressions.md).

## Pass the next loop state

`@loop` state is immutable within an iteration. A tail `@recur` supplies its next values.

```wat
(module
  (func $main (export "main") (result i64) (@effect pure)
    (@loop $sum (bind $i i32 0) (bind $total i64 0) (result i64)
      (if (result i64) (< $i 4)
        (then
          (@recur $sum (+ $i 1) (+ $total (i64.extend_i32_u $i))))
        (else (+ $total 36))))))
```

All next-state expressions are evaluated before the state changes. Recur targets the innermost Walz loop.
Use `@for` for a condition/step form or `@for-each` for each shared list element.
A for-each body returns unit. For accumulation, use a state loop or `$list.fold`.
See [loop syntax](../CHEATSHEET.md) and [list traversal](high-level-forms.md).

## Declare closure captures explicitly

Declare the closure type, capture names/types, parameters, and result. Supply capture values when constructing it.

```wat
(module
  (@closure $Adder (@capture $base i64)
    (param $n i64) (result i64) (@effect pure)
    (+ $base $n))
  (func $main (export "main") (result i64) (@effect allocates)
    (@let $add40 (@closure.new $Adder 40)
      (@closure.call $Adder $add40 2))))
```

The closure type determines the call target. Outer locals are not captured implicitly.
Capture a GC reference to share mutable state. An owned capture makes a closure single-use.
Closures can also have type parameters. See [the example](../examples/closure.walz) and [generics](generics.md).

## Type arguments and constraints

`@type.params` declares type parameters; `@where` restricts valid arguments.

```wat
(module
  (func $add (@type.params $T) (@where (Add $T))
    (param $a $T) (param $b $T) (result $T) (@effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64) (@effect pure)
    (call $add 40 2)))
```

The expected result determines T=i64 here. Write `(call (@inst $add i64) 40 2)` to specialize explicitly.
Each concrete type combination is monomorphized and checked for types, effects, and ownership. Type/alias/closure construction requires explicit type arguments.
Classes such as Add are built in; there are no user-defined class/impl declarations or runtime dictionaries.
See [generics](generics.md) and [type classes](typeclasses.md).

## Split files and select names

Save these two files in the same directory. Each file has its own scope.

```wat
;; file: math.walz
(module
  (@export (func $add))
  (func $add (param $a i64) (param $b i64) (result i64)
    (@effect pure) (+ $a $b)))
```

```wat
;; file: modules-main.walz
(module
  (@use "./math.walz" (func $add))
  (func $main (export "main") (result i64) (@effect pure)
    (call $add 40 2)))
```

Run `walz run modules-main.walz`. `@export` exposes a source API, `@use` selects it, and WAT `export` exposes a Wasm API.
Unselected names are not visible. Unreachable definitions are removed after checking; invalid unused ordinary functions are still rejected.
Use WAT `import` for host functions, with types, effects, and capabilities supplied by a binding policy.
See [static modules](static-modules.md) and [WIT bindings](wit-bindings.md).

## Move and finish owned values

Future, Stream, box, and resource owners cannot be copied. Pass them to consuming operations with `@move`.

```wat
(module
  (@use "walz:core" (type $future))
  (func $main (export "main") (result i64) (@effect suspends)
    (@let $pending (@future.ready i64 42)
      (@future.await i64 (@move $pending)))))
```

Await consumes a Future. Close an unused Future with `@future.close`; a Stream still needs closing after EOF.
Use-after-move and missing consumption are errors. Ownership is not Rust-style general reference lifetimes or exclusive ownership of every GC object.
GC reachability and closing host resources are separate concerns.

This example uses a ready value and can run in Core Wasm. Use a Core＋JSPI or Component host to await an external Promise.
Scoped child tasks are cooperative concurrency within an instance, separate from Worker parallelism.
See [runtime](runtime.md), [ownership](verification.md), and [child tasks](scoped-tasks.md).

## Continue with an application

Look up strings, bytes, lists, dictionaries, views, and JSON in the [core API](stdlib-api.md).
Use ordinary functions for shared value computations; use [hygienic macros](user-macros.md) when introducing bindings or changing syntax.
See [log statistics](../examples/apps/log-stats/README.md), [the stock ledger](../examples/apps/stock-ledger/README.md),
and [the workflow planner](../examples/apps/workflow/README.md) for larger programs.
To call Walz from JS/TS, continue with [Vite](vite.md) and [the Content Mapper](content-mapper.md).
For agent-assisted edits, the [Walz skill](agent-skills.md) supplies a compact reference and checked CLI examples.
