<a id="walz-文法チートシート"></a>

# Walz Syntax Cheatsheet

[English](CHEATSHEET.md) | [日本語](CHEATSHEET.ja.md)

A language based on standard WAT, adding typed expressions, GC data, owned values, and hygienic macros.
This sheet covers the typed core using extensions. Walz extension entry points have `@`. Standard WAT alone is accepted unchanged; WAT outside the core mixed with syntax macros is validated as a whole with opaque analysis. See [Syntax boundaries](docs/syntax.md).
For detailed semantics, see [SPEC.md](SPEC.md); for a practical example, see [Stock ledger](examples/apps/stock-ledger/README.md).

<a id="最初に覚える規則"></a>

## Rules to Learn First

- Expressions are `(operation arguments...)`. `$foo` in expression position means `(local.get $foo)`. Write `42` when its expected type is known, or explicitly `(i64.const 42)`. Strings are `(@string.lit "hello")`, or `"hello"` when the expected type is `$string`.
- A program is `(module declarations...)`. A function body is one expression. Use block for multiple operations.
- `$name` is an identifier, `"text"` a string, `;;` a line comment, and `(; ... ;)` a nestable comment.
- Arguments and operations evaluate left to right. There are no implicit numeric conversions, captures, or local reassignments.
- **`(result R)` declares a result type. `(@inst $result T E)` is a value type with success T / error E.**
- An omitted result declaration means unit, not inference of the result type from the body.
- Distinguish shared GC references from own / Future / Stream values that must be consumed.

Below, `T U E S`, `expr`, `body`, `...`, and `[optional]` are explanatory notation, not literal source text.
Each `wat` code block is a complete compilable module; `text` blocks are syntax templates.

<a id="標準-wat-と--拡張の境界"></a>

## Boundary Between Standard WAT and `@` Extensions

| Entry point | Meaning |
|---|---|
| `func`, `param`, `result`, `if`, `block`, `call` | Standard WAT functions, type declarations, and expressions; `if` has no `@` |
| `@let`, `@for`, `@for-each`, `@loop`, `@recur`, `@match`, `@try` | Walz binding/control syntax, distinct from standard `loop` / `try` |
| `(result i64)` / `(result i32 i64)` | Standard WAT result-type declarations; the latter declares standard WAT multiple results |
| `(@inst $result T E)` | Walz value type with success T / error E; a function returning it uses `(result (@inst $result T E))` |
| `(import "host" "member" ...)` / `(export "name" ...)` | Wasm instantiation import / generated-module export |
| `(@use "./file.walz" ...)` / `(@export ...)` | Selective static linking of Walz source / publication of source APIs |
| `(@use "walz:core" ...)` | Imports compiler-bundled standard types/functions individually by name |
| `@closure`, `@capture`, `@effect`, `@type.params`, `@where` | Walz declarations/annotations |
| `@macro` / `@$name` | Declaration/invocation of hygienic expression macros; standard WAT instructions are not redefined |
| `@lib` / `@world` / `@expose` / `@connect` | Static implementation / generated WIT naming / inferred Component export / explicit Component function connection |

`@` marks extension entry points. Internal `bind`, `while`, `step`, `do`, `catch`, and constraints `shared` / `nonunit` / `callable` belong to their respective entry points and have no prefix. Identifiers remain `$name`.

Known Walz `@` names are reserved and always checked. Other WAT annotations, including `@custom` and their nested payloads, stay opaque. Reserved `walz:world-v1` and Core `walz:ffi-v1` custom sections carry checked metadata, not executable syntax.

## Component entry

```text
(component
  (@world "example:calc@0.1.0" "app")
  (@lib (func $answer (result i32) (@effect pure) 42))
  (@expose "answer" $answer))
```

`import` declares an external requirement supplied by the host or another Component; `@use` only selects definitions. `walz wit-bindgen api.wit policy.json generated` generates typed declarations and bindings.json.

`walz build main.walz --wit` emits the Component and `world.wit`. An `@lib` file uses `@export` and selective `@use`; it has no runtime entry. `(@connect "module" "member" $component-function)` supplies a Walz import with checked Component types. `(@use "./worker.wasm" (component $Worker))` loads a constructor; instantiate it explicitly. See the [executable example](examples/components/main.walz) and [contracts and limits](docs/component-composition.md).

Standard WAT alone is validated and compiled unchanged. The typed core below checks types, effects, and ownership.
Extensions mixed with WAT instructions outside the core are limited to syntax-only expansion such as `@let` / `@loop` / `@for`. That path validates all Wasm and returns `analysis.opaque=true` with conservative effects. Adding `@effect pure` cannot obtain detailed analysis or ownership guarantees, and combinations with extensions requiring checks are rejected.
The typed core returns zero values (unit) or one value. A pair is also one value. Standard WAT `(result i32 i64)` uses the standard-WAT path and is not converted to typed-core pair decomposition.

<a id="ユーザー定義マクロ"></a>

For a resource interface, write `(@connect "test:resources/host" (instance $files))`.
Keep the `@resource` kind, `@borrow`, `@move` and destructor import explicit.
The compiler infers the connection contracts and supplies canonical drop;
[the resource example](examples/components/resources.walz) needs no WIT-input file or policy JSON.

Component source constructors also accept `@type.params` / `@where`.
`(instantiate $Echo<i32>)` and `(instantiate (@inst $Echo i32))` share a concrete
definition while creating independent instances. Arguments are explicit closed
value types; import core types in the caller's static scope. Resources remain
runtime instance imports. Use named Component references in generic scopes.
Visible struct/array/variant/newtype definitions can be passed as closed private
layouts; keep opaque representations and resources behind explicit interfaces.
See [generic constructors](examples/components/generics.walz) and
[typed constructors](examples/components/typed-constructors.walz).

`@component.type` reads a value type from an explicit `@connect`, without adding
authority. Use it in signatures or aliases:

```text
(@type.alias $Input (@component.type "host" (param "echo" 0)))
(@type.alias $Output (@component.type "host" (result "echo")))
(@type.alias $Field (@component.type "host" (type "item") (field "value")))
```

Selectors also traverse Option/Result/list/tuple/Future/Stream and variant
payloads. Resources still need explicit declarations and moves. These queries
can determine constructor arguments from preceding native connections, including
already specialized instances. Forward/cyclic dependencies are rejected. [Example and limits](docs/component-composition.md#connected-type-queries).

## User-Defined Macros

`@macro` declares a hygienic expression macro, invoked with `@$name`. `?name:expr/ident/type/literal` captures syntax; trailing `...` means one level of repetition. Expanded output still passes ordinary type/effect/ownership checks. [Detailed rules](docs/user-macros.md).

```wat
(module
  (@macro $twice
    (rule (?value:expr)
      (infer (?T ?value))
      (when (Add ?value))
      (@let $saved (type ?T) ?value (+ $saved $saved))))
  (@macro $with
    (rule (?name:ident ?value:expr ?body:expr)
      (bind ?name ?body)
      (@let ?name ?value ?body)))
  (func $main (export "main") (result i32)
    (@effect pure)
    (@$with $x (i32.const 21) (@$twice $x)))
  (@test "explicit binding and inferred arithmetic" (= (call $main) 42)))
```

`infer` obtains concrete type syntax from ordinary type inference. `(infer (?T expected))` queries the call's expected type; `(infer (?T ?value expected))` checks an argument using that expected type as an explicit hint. Integer widths are not guessed from values. `when` selects rules through a concrete type `(type ?value i32)` or existing type classes. `bind` declares a body where a caller-supplied name is in scope.

`(@export (macro $twice))` and `(@use "./syntax.walz" (macro $twice $double))` support selection, aliases, and re-export. `walz macro-expand file.walz` produces JSON with structured types and expansion origins; `--text` displays input single / duplicated / discarded status and syntactic occurrence counts. Execution counts depend on control flow. `walz expand` still outputs final WAT.

Type structure can be queried with forms such as `(infer (?T (option.element ?value)))`.
Selectors are `option.element` / `list.element` / `result.ok` / `result.err` / `tuple.first` / `tuple.second` /
`box.element` / `future.element` / `stream.element` / `task.element` / `array.element` / `ref.heap`.
Queries do not execute code; opaque array internals cannot be read externally. [Type information, differential fuzzing, and reproduction](docs/user-macros.md).

`bind` identifiers can repeat. Template let groups are lowered after repetition expands.

```text
(@macro $bindings
  (rule (((?name:ident ?value:expr) ...) ?body:expr)
    (bind ?name ?body)
    (@let ((?name ?value) ...) ?body)))
(@$bindings (($a (i32.const 20)) ($b (i32.const 22))) (+ $a $b))
```

Declaring the same `bind` as the callee delegates binding. New names are visible only in the specified body;
initializers use the caller's scope. An empty sequence evaluates the body; duplicate names are rejected.
When delegating a body, pass `?body` whole. Reconstructed scopes such as `(block ... ?body)` are not inferred.
To reference an earlier generated name in a macro initializer, bind the dependent value first with ordinary `@let` and pass it in.

`walz macro-expand --text examples/user-macros/main.walz` shows expansions, types, and input usage.
JSON `checking` reports whole-check passes, function-body entries, and budget-counted syntax traversals, not elapsed time or all checking operations.
Checking normally resumes from the expanded expression; if instantiation changes the type environment, the whole input is checked again.
[Multistage example](examples/user-macros/bindings.walz).

<a id="walzcore-と基本データ型"></a>

## `walz:core` and Basic Data Types

Import Walz basic types explicitly from `walz:core`. Wasm types such as `i32` are used directly.

```text
(@use "walz:core"
  (type $result) (type $option) (type $future) (type $stream)
  (type $tuple) (type $list) (type $box)
  (type $string) (type $bytes) (type $unit)
  (func $string.u64 $format))

(result (@inst $result $string i32))
(param $pending (@inst $future (@inst $result $string i32)))
(call $format (i64.const 42))
```

Apply types and generic functions with `(@inst $name T...)`. Ordinary `call $generic` can omit type arguments uniquely determined by arguments/expected type. `$string` / `$bytes` / `$unit` are aliases without type arguments, referenced by name. Aliases such as `(type $result $Outcome)` are supported too.

The shorter `$name<T, U>` spelling is equivalent in type/function/heap positions:

```text
(result $result<$tuple<i32, i32>, i32>)
(param $box (ref $Box<i32>))
(call $identity<i64> 42)
```

Nested arguments, whitespace/comments inside `<...>`, and a trailing comma are allowed. Keep `<` adjacent to the name. An exact declared/imported WAT name such as `$f<i32>` wins; `(@inst $f i32)` explicitly selects the generic. Local names and operators are unchanged. [Full rules and example](docs/generics.md#angle-application-shorthand).
Names bind only in their importing scope. There is no implicit prelude; import names in each file that uses them. Select standard functions from `$string.*`, `$bytes.*`, `$list.map` / `$list.fold`, `$string-builder.*`, and `$json.*`. `walz:core` is neither a file path nor a host import; it uses ordinary library static linking and DCE.
`@string.lit`, `@result.ok`, `@future.await`, and similar forms are fixed compiler syntax manipulating values. Refer to types using imported names and `@inst`.
See the [Standard API reference](docs/stdlib-api.md), generated from public declarations, for complete function types, constraints, and effects.
`$dict.*` provides generic text-keyed dictionaries; `$list-builder.*` builds immutable lists. Both require explicit type application for shareable nonunit elements and keep their representations opaque. See [Collections](docs/collections.md).

```wat
(module
  (@use "walz:core" (type $future))
  (func $main
    (export "main")
    (result i64)
    (@effect suspends)
    (@let
      (($pending (type (@inst $future i64)) (@future.ready i64 40))
       ($base (@future.await i64 (@move $pending)))
       ($answer (type i64) (i64.add $base 2)))
      $answer))
  (@test "standard Future" (i64.eq (call $main) (i64.const 42))))
```

<a id="ゼロコピービュー"></a>

## Zero-Copy Views

Standard types use lowercase kebab-case; related functions use `$type-name.operation-name`. `$tuple` is a two-element WIT tuple; `$box` is an owned GC box, distinct from WIT `own<resource>`. [WIT mapping](docs/naming.md).

The standard string type is `$string`. `walz:core` `$bytes-view` / `$string-view` are opaque GC types, referenced as `(ref $string-view)`.
`new` creates a whole-data view; `slice` creates a relative `start,length` view. Data is shared and only the view itself is GC-allocated, so these have `allocates`.
String ranges use UTF-8 bytes; boundaries inside characters and out-of-range accesses trap. `bytes` obtains a bytes view over the same range.
`to-bytes` / `to-string` copy the visible range and detach from the original buffer. Pass these converted values at WIT boundaries.
For details and examples, see [Views](docs/views.md) and [examples/views.walz](examples/views.walz).

For GC arrays use `(ref (@inst $array-view T $Array))`, explicitly specifying element type T and the named array type and importing functions from `walz:core`.
`(call (@inst $array-view.new i64 $Numbers) $source)` shares the whole array; `slice` shares relative element ranges; `get` reads them.
The view itself is read-only, but changes to the original mutable array remain visible. `get` has reads; `new`/`slice` have allocates.
`to-list` explicitly copies the visible range to an immutable list. See [Views](docs/views.md#任意のgc配列を共有するarray-view) for constraints, GC retention, and ABI handling.

<a id="関数型束縛"></a>

## Functions, Types, and Bindings

```wat
(module
  (func $add
    (param $a i64)
    (param $b i64)
    (result i64)
    (@effect pure)
    (i64.add (local.get $a) (local.get $b)))

  (func $main
    (export "main")
    (result i64)
    (@effect pure)
    (@let $answer (call $add (i64.const 40) (i64.const 2))
      (local.get $answer)))
  (@test "answer" (i64.eq (call $main) (i64.const 42))))
```

| Type | Meaning |
|---|---|
| `i32 i64 f32 f64` | Wasm numeric types; integer add/subtract/multiply wrap, integer division by zero and similar operations trap; instruction `_s` / `_u` specifies signedness |
| `v128` | Wasm 128-bit value; use standard names for `v128.const`, splat, lane operations, arithmetic/comparison/shuffle, etc.; [SIMD](docs/simd.md) |
| `(ref $T)` / `(ref null $T)` | Non-null/nullable GC references; nominal types |
| `$bytes` / `$string` | Immutable byte sequence / valid UTF-8; internal references cannot be manipulated or compared |
| `(@inst $list T)` | Immutable list of shared values |
| `(@inst $option T)` / `(@inst $result T E)` | None/some and ok/err value types |
| `(@inst $tuple T U)` | Two-component value; shared when both components are shared |
| `(@inst $box T)` / `(@inst $future T)` / `(@inst $stream T)` | Owned values that must be consumed; does not mean exclusive ownership of GC references |
| `$unit` | Result with no value; not stored in function parameters or ordinary fields |

```text
(func $f (param $x T)... [(result R)] [(@effect ...)] body)
(@let $x [(type T)] initializer body) ;; 注釈省略時は初期値の型。不変の字句束縛
(@let (($a [(type T)] initial-a) ($b [(type U)] initial-b) ...) body)
(local.get $x)                      ;; 式位置では $x とも書ける
(call $f arg...)
(block [(result R)] unit-expr... final-expr)
(if [(result R)] i32-condition (then expr) (else expr))
(drop shared-value)                 ;; unit の式へ。所有値は drop しない
(nop)                              ;; unit
(unreachable)                      ;; trap。Wasm 例外とは異なる
```

If/block/match without result declarations also have type unit. For unit results, omit the result declaration instead of writing `(result $unit)`.
Every block expression except the last must have type unit.
Discard temporary integers with `(drop ...)`; explicitly update mutable GC fields/arrays for state changes.

Multi-binding `@let` evaluates initializers once each from left to right. Later initializers/body can refer to earlier bindings, but forward references are not allowed. Same-name rebinding shadows the old name after its initializer evaluates. Individual annotations can be omitted. Semantics match nested `@let`; this does not decompose Wasm multiple results.
Use existing `(@let ($a $b) subject body)` for pair decomposition. Both forms check types/effects/ownership; owned values require explicit move and consumption.

```text
(@let {:category $key :amount $amount} $row
  (call $add $key $amount))

(@for-each ($key $amount) $entries
  (call $add $key $amount))

(@struct.update $State $state
  ($accepted $old (+ $old 1)))
```

Field decomposition selects needed fields from a non-null shared struct, evaluates the subject once, and reads/binds values to the body in written order. `@for-each` traverses lists of shareable nonunit elements; body and overall expression have type unit. Patterns can be a single name, tuple, or fields; use outer block/br to exit. `@struct.update` reads a mutable field once and writes the value computed from its old value once. It returns unit and is not atomic. [Rules and examples](docs/high-level-forms.md).

```wat
(module
  (func $main (export "main") (result i32)
    (@let $foo (type i32) 41
      (i32.add $foo 1))))
```

The type of `1` comes from the operation, parameter, result, field, `(type T)` annotation, or similar context. There is no default integer width: `(@let $x 1 ...)` and `(drop 1)` are ambiguous errors. `$foo` only reads an existing local, without implicit move/conversion. [Shorthand rules](docs/core-expressions.md#ローカル参照と数値の省略形).

<a id="基本型の型クラス"></a>

## Type Classes for Basic Types

| Class | Supported types | Binary operators |
| --- | --- | --- |
| `Add` | i32 / i64 / f32 / f64 | `+` |
| `Sub` | i32 / i64 / f32 / f64 | `-` |
| `Mul` | i32 / i64 / f32 / f64 | `*` |
| `Div` | i32 / i64 / f32 / f64 | `/`; signed division for integers |
| `Eq` | i32 / i64 | `=` / `!=` |
| `Ord` | i32 / i64 | `<` / `<=` / `>` / `>=`; signed comparison |
| `Logical` | i32 | `&&` / `\|\|`; short-circuits and returns 0/1 |
| `Bitwise` | i32 / i64 | `&` / `\|` / `^`; evaluates both operands for bitwise operations |
| `Shift` | i32 / i64 | `<<` / `>>` / `>>>`; left, signed right, zero-fill right shift |

```wat
(module
  (func $four (result i32) (+ 1 3))
  (func $add (@type.params $T) (@where (Add $T))
    (param $a $T) (param $b $T) (result $T) (@effect pure)
    (+ $a $b))
  (func $main (export "main") (result i64)
    (call (@inst $add i64) 40 2))
  (@test "literal plus" (= (call $four) 4))
  (@test "short circuit" (|| (= (call $four) 4) (unreachable)))
  (@test "masked shift" (= (i64.const 1) (<< (i64.const 1) 64))))
```

Fixed symbolic operators have no `@`. `+` derives its type from an expected type or known operand type. A comparison's i32 result does not determine operand width; when all operands are bare numbers, specify one explicitly, as in `(< (i64.const 1) 3)`. There are no user-defined class/impl declarations; operations expand directly to Wasm instructions after monomorphization. [Detailed rules](docs/typeclasses.md).
Inputs to `&&`/`||` are i32, like conditions. Right operands are statically checked but execute only when needed. Shift counts have the same type as the left operand and use the low 5 bits (i32) / 6 bits (i64), as in Wasm. `(Logical $T)`, `(Bitwise $T)`, and `(Shift $T)` are allowed in `@where` too.
`Sub`, `Mul`, and `Div` are also constraints. Integer `/` rounds toward zero and traps on zero divisor or minimum/-1. Floating-point `/` follows Wasm floating-point division.

Bare strings are also allowed in parameters, results, fields, list elements, and annotated lets with expected type `$string`. Let initializers without an expected type need `(type $string)` or `@string.lit`. No conversion to another type occurs. [Option/Result functions and string rules](docs/option-result.md).

<a id="gc-データと-enum"></a>

## GC Data and Enums

```text
(type $Cell (struct (field $value (mut i64))))
(struct.new $Cell value)
(struct.get $Cell $value object)
(struct.set $Cell $value object value)

(type $Ints (array (mut i64)))
(array.new $Ints initial-element length)
(array.new_fixed $Ints 2 first second)
(array.get $Ints array index)
(array.set $Ints array index value)
(array.len array)
(type $Octets (array (mut i8)))      ;; packed i8/i16 storage; values are i32
(array.get_u $Octets array index)    ;; or get_s for signed extension
(array.copy $Octets $Octets dst dst-index src src-index length)
(ref.null $Cell)
(ref.as_non_null nullable)           ;; null なら trap。暗黙の型絞り込みはない
(ref.is_null reference)
(ref.eq reference reference)         ;; 構造比較でなく参照同一性
```

Named construction requires every field. Duplicate, missing, and unknown names are rejected; initializers evaluate once each in written order.
`{}` and bare literals are independent sugar preserving type/effect/ownership checks. [Details](docs/struct-construction.md).

```text
(@struct.new $User ($name (@string.lit "Alice")) ($age (i32.const 30)))
(@struct.new $User ($name "Alice") ($age 30))
(@struct.new $User {:name (@string.lit "Alice") :age (i32.const 30)})
(@$User::{:name "Alice" :age 30})
(@struct.new (@inst $Box i64) {:value 42})
```

```wat
(module
  (type $Item (@variant (none) (count i64)))
  (func $get
    (param $item (ref $Item))
    (result i64)
    (@match $Item (result i64)
      (local.get $item)
      (@case none () (i64.const 0))
      (@case count ($n) (local.get $n))))
  (@test "variant"
    (i64.eq (call $get (@variant.new $Item count (i64.const 42))) (i64.const 42))))
```

Cover each variant exactly once with explicit cases or a default. Cases without payloads still require `()`. Types, arity, and exhaustiveness are checked.
A final `(@default body)` may cover remaining variants without payload bindings. It must be unique, final, and nonredundant; omitted owned or borrowed payloads are rejected. Defaults retain type/effect/ownership checks and runtime tag/subtype traps. Write `@case`/`@default` explicitly; standard Component WAT `case` declarations retain their native spelling.

Ordinary enums use `(ref $Item)`; Option/Result/pair are value types without direct reference manipulation.

## Option / Result / Pair

```text
(@option.none T)
(@option.some T value)
(@result.ok T E value)
(@result.err T E error)
(@tuple.new T U first second)

(@match (@inst $option T) (result R) subject
  (@case none () no-value) (@case some ($value) with-value))
(@match (@inst $result T E) (result R) subject
  (@case ok ($value) success) (@case err ($error) failure))
(@match (@inst $tuple T U) (result R) subject
  (@case pair ($first $second) body))
```

Omit value arguments and payload bindings for unit components, e.g. `(@result.ok $unit i32)` and `(@case ok () body)`.
A function returning Result uses `(result (@inst $result T E))`: the outer form is a declaration, the inner a value type.

The following returns pair state in a Result. In match `(result (@inst $result ...))`, too, the outer form is a declaration and the inner a value type.
This explicit form writes types at each construction/decomposition site. Type aliases and shorthands are also available.

```wat
(module
  (@use "walz:core" (type $tuple) (type $result))
  (func $increment
    (param $state (@inst $tuple i64 i32))
    (result (@inst $result (@inst $tuple i64 i32) i32))
    (@effect pure)
    (@match (@inst $tuple i64 i32) (result (@inst $result (@inst $tuple i64 i32) i32))
      (local.get $state)
      (@case pair ($id $count)
        (@result.ok (@inst $tuple i64 i32) i32
          (@tuple.new i64 i32 (local.get $id) (i32.add (local.get $count) (i32.const 1))))
      )))
  (@test "pair state"
    (@match (@inst $result (@inst $tuple i64 i32) i32) (result i32)
      (call $increment (@tuple.new i64 i32 (i64.const 7) (i32.const 41)))
      (@case ok ($state)
        (@match (@inst $tuple i64 i32) (result i32)
          (local.get $state)
          (@case pair ($id $count) (i32.eq (local.get $count) (i32.const 42))))
      )
      (@case err ($error) (i32.const 0))))
)
```

<a id="resultlet成功時だけ続ける"></a>

### Result.let: Continue Only on Success

```text
(@result.let ($value) (@inst $result T E) U subject body)
(@result.let ($value) subject body)    ;; 入力型と出力成功型を省略する形
```

Subject is result<T,E>, body is result<U,E>, and the entire expression is result<U,E>.
In both forms, the final two expressions are subject and success body. Omitting types preserves evaluation order, scope, and err propagation.
Argument positions are as follows. **Bare U is only the output success type**, not the input or error type.

| Position | Specifies |
|---|---|
| `($value)` | Name for the subject's success payload, visible only in body |
| `(@inst $result T E)` | Input subject's value type |
| `U` | Success type of body and entire expression; error type E comes from input |
| `subject` / `body` | Input expression / expression evaluated only on success |

For T=unit, use `()` instead of `($value)`. Initial-version E must be shared.
On err, body is skipped and the same E err becomes this expression's result. **This is not implicit function return.**

`()` for unit input means “do not name the success payload.” The success continuation body is still required.

```wat
(module
  (@use "walz:core" (type $result) (type $unit))
  (func $after-unit
    (param $input (@inst $result $unit i32))
    (result (@inst $result i64 i32))
    (@effect pure)
    (@result.let ()
      (local.get $input)
      (@result.ok (i64.const 42))))
  (@test "unit success continues"
    (@match (call $after-unit (@result.ok $unit i32))
      (@case ok ($n) (i64.eq (local.get $n) (i64.const 42)))
      (@case err ($e) (i32.const 0))))
  (@test "unit error propagates"
    (@match (call $after-unit (@result.err $unit i32 (i32.const 7)))
      (@case ok ($n) (i32.const 0))
      (@case err ($e) (i32.eq (local.get $e) (i32.const 7))))
  ))
```

```wat
(module
  (@use "walz:core" (type $result))
  (func $convert
    (param $input (@inst $result i64 i32))
    (result (@inst $result i32 i32))
    (@effect pure)
    (@result.let ($n) (@inst $result i64 i32) i32
      (local.get $input)
      (@result.ok i32 i32 (i32.wrap_i64 (local.get $n))))
  )
  (@test "result mapping"
    (@match (@inst $result i32 i32) (result i32)
      (call $convert (@result.ok i64 i32 (i64.const 42)))
      (@case ok ($n) (i32.eq (local.get $n) (i32.const 42)))
      (@case err ($e) (i32.const 0))))
)
```

<a id="future--stream-と終了処理"></a>

## Future / Stream and Cleanup

```text
(@future.ready T value)
(@future.await T (@move $future))       ;; future を消費し T を返す
(@future.close T (@move $future))
(@stream.empty T)
(@stream.cons T value rest-stream)    ;; rest-stream も消費する
(@stream.next T (@move $stream))       ;; pair<stream<T>, option<T>>
(@stream.close T (@move $stream))
```

Await/next always have suspends. Even next at EOF returns an endpoint, so a final close is required.
Payloads must be shareable values. Endpoints themselves cannot be stored in shared GC fields.
Ordinary core execution handles ready data; Core+JSPI / Component also connect to Promises and external Streams.

<a id="streamfold-result失敗時も閉じる反復"></a>

### Stream.fold-result: Iteration That Closes on Failure Too

```text
(@stream.fold-result T S E source initial ($state $item) body)
(@stream.fold-result source initial ($state $item) body)
```

Source is stream<result<T,E>>, initial is S, and body is result<S,E>. The whole expression is result<S,E>.
The short form omits only leading type arguments T / S / E; source, initial, bindings, and body remain in the same order.
Source then initial evaluate once each. S is shareable, nonunit state.
EOF closes and returns ok(state); input err/body err closes and returns err(E).
For T=unit the binding is only `($state)`. Body expands to loop/match within the same function, generating no closure.
This is not finally for arbitrary throws. Existing task scopes handle cancellation acknowledgment.

| Completion reason | Fold and caller behavior |
| --- | --- |
| EOF | Closes input and returns ok with final state |
| Input or row-processing err | Closes input and returns err; the outer success body does not execute |
| Cancellation from JS | The task scope created by host `start` aborts waiting operations and acknowledges after cleanup completes; does not convert to Result err |
| Wasm throw / trap | Separate from Result; fold does not guarantee close for arbitrary exceptions; traps cannot be caught |

`@task.scope (result T) body` groups language-side child tasks and waits for completion on exit. `@task.cancel T handle` waits for cancellation/cleanup of the child and descendants, leaving parent and siblings running. `@task.join-timeout T ms handle` returns `some` on completion or `none` after timeout cleanup. Both operations consume the handle. See [Child-task contracts](docs/scoped-tasks.md).

The JS host also has a task scope per public call. `task.cancel()` returns a Promise waiting for cleanup after the request; `task.result` rejects with `TaskCancelledError` on cancellation. Cancellation is cooperative at waiting points; it does not forcibly stop CPU-only work or roll back existing changes. See [Task cancellation](docs/runtime.md#タスクの取消し) for examples.

<a id="型の-alias-と省略形"></a>

## Type Aliases and Shorthand

`(@type.alias $State (@inst $tuple i64 i32))` is transparent type shorthand. Write `$State` in type positions.
It adds no GC object or nominal identity. Do not use it as a heap-type name such as `(ref $State)`.
Forward references are allowed; cycles are forbidden. File-level publication is `(@export (type $State))`; use works as for types.

When an expected type follows from function results, arguments, explicit let types, or similar context, value construction need not repeat type arguments.
Add annotations when it cannot be determined. Omitted result declarations of functions/if/block/explicit match remain unit.

```wat
(module
  (@use "walz:core" (type $tuple) (type $result))
  (@type.alias $State (@inst $tuple i64 i32))
  (func $increment
    (param $state $State)
    (result (@inst $result $State i32))
    (@let ($id $count) (local.get $state)
      (@result.ok
        (@tuple.new (local.get $id) (i32.add (local.get $count) (i32.const 1))))
    ))
  (@test "short state"
    (@match (call $increment (@tuple.new (i64.const 7) (i32.const 41)))
      (@case ok ($state)
        (@let ($id $count) (local.get $state)
          (i32.eq (local.get $count) (i32.const 42))))
      (@case err ($error) (i32.const 0))))
)
```

| Shorthand | Type determination |
|---|---|
| `(@result.ok value)` / `(@result.err error)` | Expected Result type; omit the value argument for unit payloads |
| `(@tuple.new first second)` | Expected pair type; omit unit-component value arguments |
| `(@option.some value)` / `(@option.none)` / `(@box.new value)` | Expected Option / Own type |
| `(@let ($first $second) subject body)` | Evaluates/decomposes the subject pair once; omit unit-component names |
| `(@match subject (@case ...)... )` | Subject type and expected output type; long typed form is also allowed |
| `(@result.let ($value) subject body)` | Subject T/E and U locally determined by expected type or body |
| `(@stream.fold-result source initial ($state $item) body)` | Source T/E and S from initial or expected type; body is result<S,E> |

Shorthand preserves evaluation order, consumption obligations, effects, and opaque checks. Result.let E is shared and identical in input/output. Fold E is also shared; S must be shareable and nonunit.
To decompose a pair containing own components with pattern let, use `(@move $tuple)` and consume the extracted owned values too.
There are no arbitrary type holes, whole-program inference across functions, or implicit move/capture.

<a id="所有値クロージャeffect"></a>

## Owned Values, Closures, and Effects

Generic functions, types, aliases, and closures declare `(@type.params $T ...)` immediately after the name
and instantiate with `(@inst $name T...)`. Ordinary function `call` can omit uniquely determined type arguments; types, effects, and ownership are still checked after instantiation.
Concrete closure types can also be passed. [Details and map/fold](docs/generics.md).

```wat
(module
  (func $identity
    (@type.params $T)
    (@where (shared $T) (nonunit $T))
    (param $x $T)
    (result $T)
    (@effect pure)
    (local.get $x))

  (func $main
    (export "main")
    (result i64)
    (@effect pure)
    (call (@inst $identity i64) (i64.const 42))))
```

Placing `(@where (shared $T) (nonunit $T))` immediately after type.params constrains instantiated type arguments.
Shared does not mean immutable. Callbacks can use
`(callable $F (param $T) (result $U) (@effect pure))`.
This requires shared, repeat-callable closures with matching parameters/results; effects are inferred from the body.
Omitting effects imposes no effect restriction. Even unused applications are checked before DCE.
Explicit application is `(@inst $name T...)`: for functions, `(call (@inst $identity i64) value)`; generic types, `(ref (@inst $Box i64))`; generic aliases, `(@inst $Values i64)` in a type position.
Function calls derive type arguments from arguments, expected type, and callable signatures. Ambiguity yields `E_GENERIC_INFERENCE`. Numeric widths are not guessed and implementations are not searched. Type/effect/ownership checks apply to instantiated bodies; an unapplied generic body is not guaranteed valid for arbitrary type arguments.

```text
(@box.new T value)
(@move $owner)                       ;; 所有値を消費する位置に渡す
(@box.close (@move $owner))            ;; 包みを消費。OS resource の destructor ではない

(@closure $Adder (@capture $base i64) (param $x i64) (result i64)
  (i64.add (local.get $base) (local.get $x)))
(@closure.new $Adder (i64.const 40))
(@closure.call $Adder closure (i64.const 2))
```

Local.get, drop, and reuse after move are forbidden for owned values. Option/Result/pair with an owned component are themselves owned.
Own does not prohibit aliases of payload GC references. GC collection and external-endpoint cleanup are separate.
Closures declare every capture and receive capture values at construction. Closures with owned captures can be called once only.

```wat
(module
  (@closure $Adder
    (@capture $base i64)
    (param $x i64)
    (result i64)
    (i64.add (local.get $base) (local.get $x)))

  (func $main
    (export "main")
    (result i64)
    (@let $add40 (@closure.new $Adder (i64.const 40))
      (@closure.call $Adder (local.get $add40) (i64.const 2))))
  (@test "explicit capture" (i64.eq (call $main) (i64.const 42))))
```

| Effect | Meaning |
|---|---|
| `pure` | Empty semantic-effect set below; does not prove termination, no traps, or no physical allocation |
| `reads writes` | Mutable GC data reads/writes; conservative bounds including local work arrays |
| `allocates` | Creation of GC values with observable identity, distinct from storage allocation for value types |
| `host suspends` | Host effects / suspension boundaries, checked separately |
| `(throws $Tag)` | Wasm exceptions that may escape the function |

Effect annotations are upper bounds checked against actual bodies. Omission means inference. Opaque public types do not hide mutation-method effects.

<a id="ループwasm-例外"></a>

## Loops and Wasm Exceptions

```text
(@loop $go (bind $i i32 initial)... (result R) body)
(@recur $go next-value...)            ;; 最内側 loop の末尾だけ。同時更新
(@for $go (bind $i i32 initial)...
  (while condition) (step next-value...) unit-body)

(tag $Error (param i32))
(@try (result R)
  (do expression)
  (catch $Error ($code) handler)
  (catch_all handler))
(throw $Error payload...)
```

Try/catch is sugar for Wasm exceptions. Catch_all does not catch division-by-zero, out-of-bounds, or other traps.
Try/throw within functions with owned values is restricted in the initial version; use Result for those failures.

<a id="モジュール不透明な公開型"></a>

## Modules and Opaque Public Types

`(@use "./math.wasm" (component $Math))` selects a binary constructor. Use explicit `instantiate`, `alias`, and `@connect` for state and imports; function/type selection from `.wasm` is rejected. Async and owned ports use native Component contracts. Unused explicit instances retain initialization. [Examples and limits](docs/wasm-use.md).

```wat
;; stock.walz: 1ファイルが1モジュール、宣言は既定で非公開
(module
  (@export (opaque $State) (func $new) (func $get))
  (type $State (struct (field $n i32)))
  (func $new (result (ref $State)) (struct.new $State (i32.const 42)))
  (func $get (param $s (ref $State)) (result i32)
    (struct.get $State $n (local.get $s))))
```

```wat
;; main.walz: 対象と別名を明示して選択
(module
  (@use "./stock.walz" (type $State $Alias) (func $new) (func $get $read))
  (func $main (export "main") (result i32) (call $read (call $new))))
```

There is no `@namespace`. File `@use` uses relative paths beginning `./` or `../`; built-ins use package names such as `walz:core`.
Dots are identifier characters, with no namespace search. Only selected names are bound; types/definitions from the same file are shared. Aliases can also be re-exported with `@export`.
`@export (type $T)` also exposes representation; `@export (opaque $T)` exposes only the type. Opaque construction, field/array operations, and match are limited to the declaring file, including through aliases.
`@public` is synonymous with `@export`. Native WAT `(import "host" "member" ...)` and `(export "main" ...)` serve the Wasm ABI, separately from static `@use` / `@export`.
`@include "relative.walz"` textually inserts declarations without creating scope. Unused definitions are eliminated during generation after checking.
See [Boundaries, DCE, and unsupported scope](docs/static-modules.md).

<a id="bytes--text-とライブラリ"></a>

## Bytes / Text and Libraries

| Expression/function | Result |
|---|---|
| `(@bytes.new i32-expr...)` | Immutable bytes |
| `(@bytes.from-array $Array array [start length])` | Immutable snapshot of all/part of an i32, i8 or i16 array; selected unsigned values outside 0..255 trap |
| `(@bytes.copy-to-array $Array dst at bytes start length)` | Bulk copy into mutable packed i8 storage; bounds violations trap |
| `(@bytes.read-i32-le b offset)` / `$bytes.read-i32-le(b,offset)` | Fixed-width read; formats i16/u16/i32/i64/f32/f64, each le/be; incomplete ranges trap |
| `$bytes-view.read-i32-le(view,offset)` | Same reads relative to the view; checks the entire visible range |
| `(@array.write-i32-le $Array dst offset value)` | Mutable packed i8 only; formats i16/i32/i64/f32/f64, each le/be; checks the full range before writes |
| `(@bytes.len b)` / `(@bytes.get b index)` | Length / element in 0..255; unsigned index; out of bounds traps |
| `(@bytes.slice b start length)` | Copied bytes; out of bounds traps |
| `(@string.lit "日本語")` / `(@string.byte-len t)` | Text / UTF-8 byte length |
| `(@string.bytes t)` / `(@string.from-bytes b [start length])` | Bytes / option<text>; invalid UTF-8 gives none; explicit ranges validate before copying |
| `(@bytes.is-utf8 b [start length])` / `$bytes-view.is-utf8(view)` | Strict UTF-8 validation without a payload copy; invalid ranges trap |
| `$string-view.from-bytes-view(view)` | option<string-view> sharing the validated byte range; materialize with `$string-view.to-string` |
| `$i32.checked-add-u(a,b)` / `$i32.checked-mul-u(a,b)` | option<i32>; unsigned bit patterns, none on overflow; matching `$i64.*` helpers return option<i64> |
| `$i32.checked-align-up-u(value,alignment)` | option<i32>; nonzero power-of-two alignment, none on invalid alignment/overflow; also `$i64.*` |
| `$string.concat(text,text)` | Text concatenation |
| `$string.u64(i64)` / `$string.s64(i64)` | Unsigned/signed decimal conversion of a bit pattern |
| `$json.from-string(text)` | Constructs a JSON string value (parse documents with `$json.parse`) |
| `$json.stringify(value)` | Outputs a JSON value as JSON text |

The standard-library entry point is consistently `walz:core`. For example, import with `(@use "walz:core" (type $string) (func $json.from-string) (func $json.stringify))`, then output a JSON string literal with `(call $json.stringify (call $json.from-string text))`. Functions using internal work arrays have reads/writes/allocates.
`walz/core/*.walz` contains bundled implementation files, distinct from `@include "program.walz"` inserting user declarations. `@include` inserts text into the same scope; it does not implicitly provide standard names.

Use `$bytes.find-byte`, `$string.find`, `$string.split-once`, and `$string.parse-u64` / `$string.parse-s64` for search, splitting, and integer parsing.
`parse-u64` checks the entire input for ASCII digits and returns some(i64) in the unsigned 64-bit range. Empty input, negatives, `+`, whitespace, and overflow yield none. Leading zeroes are allowed. Success is an i64 bit pattern; use `string.u64` / `builder.u64` to convert back to unsigned decimal.
Positions use UTF-8 bytes. `$bytes.parse-u64(bytes,start,length,limit)` accepts application-specific bounds too; invalid input gives none.
See [All APIs and boundary rules](walz/core/README.md). JSON objects support automatic key/value escaping and delimiter output.

```wat
(module
  (@use "walz:core" (type $string))
  (@use "walz:core"
    (func $json.object-builder.new)
    (func $json.object-builder.string)
    (func $json.object-builder.u64)
    (func $json.object-builder.finish))
  (func $record
    (export "record")
    (param $name $string)
    (param $count i64)
    (result $string)
    (@effect reads writes allocates)
    (@let $o (call $json.object-builder.new)
      (block
        (result $string)
        (call $json.object-builder.string (local.get $o) (@string.lit "name") (local.get $name))
        (call $json.object-builder.u64 (local.get $o) (@string.lit "count") (local.get $count))
        (call $json.object-builder.finish (local.get $o))))
  ))
```

`$json.object-builder.finish` returns contents and restores an empty object. For text fragments, use `$string-builder.new` / `$string-builder.text` / `$string-builder.finish` from `walz/core/builder.walz`.

<a id="確認コマンド"></a>

## Verification Commands

Create a project with the installed CLI; no compiler checkout is needed.
Execution needs Wasmtime with Wasm GC. Benchmarks and Core＋JSPI execution need Node.js 24+.

```sh
walz new app
cd app
walz check src/main.walz
walz run                           # 42
walz build                         # target/main.wasm
walz test
walz bench --list
walz bench --samples 10
walz types src/main.walz
walz expand src/main.walz
walz fmt-check src/main.walz
```

For a JS bundle, use `walz build --target core-js -o dist/app`. Use `walz run --target core-js` to inspect portable return values;
Core Wasm alone does not turn GC references into strings. With an I/O program, `--stdio` explicitly enables standard I/O,
`--input input.txt` supplies its input, and `--output output.txt` redirects its output. These flags do not turn a return value into raw text.
Application failures represented by a Result err produce an error exit from `walz run`.

`(@test "name" i32-expr)` fails for zero and succeeds otherwise. `@law` is a different contract:
it checks all inputs of a supported pure integer expression using Lean.
A `@bench` in `*_bench.walz` is discovered by `walz bench`; its iteration parameter and result are optional.
Return the measured value and vary inputs. See [benchmarking](docs/benchmarking.md) for timing boundaries, warmup, and JSON reports.
The existing text I/O capabilities use stream<result<text,i32>> for read-lines and future<result<unit,i32>> for write-line.

See [development](docs/development.md) for compiler-checkout tasks and the [runtime](docs/runtime.md) for lower-level host APIs.
Whole-program type inference, user-defined traits, procedural macros, general borrow lifetimes, and unrestricted WASI bindings are not provided.

<a id="数値平坦な-result-連鎖newtype早期脱出"></a>

## Numbers, Flat Result Chains, Newtypes, and Early Exit

Scalar conversions, saturating conversions, rounding, sqrt, min/max, and reinterpretation use standard WAT instruction names unchanged.
`i32.trunc_f64_s` traps on out-of-range values and NaN; `i32.trunc_sat_f64_s` saturates.

```text
(type $UserId (@newtype i64))
(@newtype.wrap $UserId scalar)        ;; 型検査後に消去。基底はi32/i64/f32/f64
(@newtype.unwrap $UserId id-value)    ;; 異なるnewtypeや数値への暗黙変換はない

(@result.do
  ($a first-result)
  ($b second-result)
  (() unit-success-result)
  final-result)

(return [value])
(block $done [(result R)] body...)
(br $done [value])
(br_if $done [value] condition)       ;; falseならvalueが残る
```

`@result.do` has one or more bindings and a final Result expression. Each evaluates once from left to right; the first err skips the remainder and returns that expression's err. All error types are identical. `$a` is visible to later bindings; unit success binds to `()`. It does not implicitly return from the function.

```wat
(module
  (@use "walz:core" (type $result))
  (type $Id (@newtype i32))
  (func $compute (result (@inst $result $Id i32)) (@effect pure)
    (@result.do
      ($a (@result.ok i32 i32 (i32.const 40)))
      ($b (@result.ok i32 i32 (i32.const 2)))
      (@result.ok $Id i32
        (@newtype.wrap $Id (i32.add (local.get $a) (local.get $b))))))
  (func $main (export "main") (result i32) (@effect pure)
    (@match (call $compute)
      (@case ok ($id) (return (@newtype.unwrap $Id (local.get $id))))
      (@case err ($code) (i32.const 0))))
  (@test "newtype and flat results" (i32.eq (call $main) (i32.const 42))))
```

Named-block branch values and return values must match declared result types. `br_if` evaluates its value before its condition.
Return, branch, and iteration exiting `@using` evaluate values first, then close only exited scopes in reverse order.
Owned values can coexist with try/catch, but must be consumed on every path. A later argument throwing and leaving an earlier owned argument behind is also rejected.
Newtypes distinguish types within Walz and become underlying numbers at JavaScript/WIT boundaries. Range checks and JavaScript brands are not added automatically.
[Detailed restrictions and examples](docs/core-expressions.md).

<a id="共有メモリと-atomics"></a>

## Shared Memory and Atomics

```text
(import "env" "memory" (memory $shared 1 2 shared))
(i32.atomic.load $shared offset=0 align=4 address)
(i64.atomic.store $shared address value)
(i32.atomic.rmw.add $shared address value)
(i64.atomic.rmw.cmpxchg $shared address expected replacement)
(memory.atomic.wait32 $shared address expected timeout-i64)
(memory.atomic.notify $shared address count-i32)
(atomic.fence)
(memory.size $shared)
(memory.grow $shared pages-i32)
```

Atomic load has `reads synchronizes`, store `writes synchronizes`, and RMW `reads writes synchronizes`. Wait has `reads synchronizes blocks`; notify/fence have `synchronizes`. Wait timeout is in nanoseconds; results are notified=0 / not-equal=1 / timed-out=2. `blocks` is distinct from JSPI `suspends`.
Addresses are i32; values are i32/i64 according to the instruction. GC refs and owned values cannot be stored. Existing move checks remain, but exclusive region ownership and race freedom are not proved.
Use Core / Core+JSPI to pass shared memory to the host. [All instructions, types, constraints, and Worker examples](docs/threads.md).

<a id="固定-atomic-セルの検証モード"></a>

## Verification Mode for Fixed Atomic Cells

```text
(@race-free)
(import "env" "memory" (memory $m 1 1 shared))
(@atomic.cell $counter $m i32 0)
(i32.atomic.rmw.add (@atomic.at $counter) value-i32)
```

Memory is single, fixed-size, and shared; cells have nonoverlapping fixed i32/i64 widths. Raw addresses, narrow accesses, extra offsets, size/grow, and host-function imports are rejected. `@atomic.at` cannot be extracted as an integer. Cells support aliases through `@export (cell $name)` / `@use "./file.walz" (cell $name $alias)`.
Native builds execute the Lean kernel. Assuming host contract compliance, proofs cover absence of data races, identical cell widths, and bounds, not lost updates or deadlock. Core+JSPI host sharing requires `atomicHostContract: 'walz-atomic-cells-v1'`. [Specification and proof scope](docs/atomic-cells.md).

<a id="読み取り専用の共有領域"></a>

## Read-Only Shared Regions

```text
(@readonly.region $index $m byte-offset positive-byte-length)
(@readonly.load $index u8 relative-byte-offset-i32)
(@readonly.load $index i32 relative-byte-offset-i32)
(@readonly.load $index i64 relative-byte-offset-i32)
(@readonly.load $index v128 relative-byte-offset-i32)
```

Offsets evaluate once, bounds are checked within the region, and reads are nonatomic. Requires `reads`. Combined with `@race-free`, reads separated from atomic cells are verified under a host contract forbidding writes after publication. Additional defined private memories are allowed. Core+JSPI specifies `readonlyHostContract: 'walz-readonly-regions-v1'`. [Details](docs/readonly-regions.md). The [General Worker host](docs/worker-host.md) is used explicitly from JS; it does not make `@task.spawn` run on a separate thread.

<a id="option--result-の標準関数"></a>

## Standard Option / Result Functions

Import explicitly with `(@use "walz:core" (func $option.ok-or) (func $result.map-err))`.
`(call (@inst $option.ok-or T E) optional error)` maps some→ok and none→err. Error is always evaluated eagerly beforehand.
`(call (@inst $result.map-err T E U $Closure) result-value mapper)` invokes the closure once only on err and preserves ok.
T/E/U must be shared/nonunit; the closure is E→U. Use `@match` for unit or owned payloads. [Contracts and examples](docs/option-result.md).

## Optional Watz printing

Watz match clauses use `case`/`default`; canonical Walz keeps `@case`/`@default`.

`walz print file.walz` prints canonical Walz S-expressions. Pass `--format watz` for a reversible review view using `func`, `@export("name")`, typed immutable `let`, direct `match`/`if` expressions, and structural `wat.*`/`walz.*` calls. `walz print file.watz` prints the view back as Walz. The module wrapper is optional. This view adds no types or runtime semantics. See [mapping and examples](docs/review-syntax.md).
