# Distinguishing Walz Extensions with `@`

Syntax entry points added by Walz use `@`. Unprefixed `import`, `export`, `loop`, `try`, and `result` retain WAT meanings. `$name` identifiers, comments, and string contents are unchanged.
The fixed symbolic operators `+`, `-`, `*`, `/`, `=`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `&`, `|`, `^`, `<<`, `>>`, and `>>>`, and expression-position local/numeric shorthand, are unprefixed sugar.

Known Walz `@` names are reserved and enter the checked extension path. They are never discarded as WAT annotations, including declarations with invalid arguments. Other annotations, such as `@custom` and application metadata, retain WAT behavior; their complete payloads remain opaque even when they contain extension-looking forms. WIT identifier escaping and URI percent encoding still use `%`.

| Purpose | Syntax |
| --- | --- |
| Expressions/control structures | `@let`, `@for`, `@loop`, `@recur`, `@match`, `@case`, `@default`, `@try`, `@result.do` |
| Value operations | `@list.new`, `@string.lit`, `@result.ok`, `@future.await`, etc. |
| Struct construction | `(@struct.new $T ($field expression) ...)`, also `{}` field groups and `(@$T::{...})` shorthand |
| Value types | `$string`, `$bytes`, `$unit`, `(@inst $list T)`, `(@inst $option T)`, `(@inst $result T E)`, etc. |
| Declarations/annotations | `@closure`, `@capture`, `@effect`, `@newtype`, `@type.alias`, `@type.params`, `@where`, `@test`, `@law` |
| Static modules | `@include`, `@use`, `@export`, `@public` |
| Type application | `(@inst $name T...)`, shared by functions, types, aliases, and closures |
| User-defined syntax | Declared with `@macro`, called with `@$name`; `rule`, `infer`, `when`, and `bind` are syntax inside declarations |

Inside extensions, `bind`, `while`, `step`, `catch`, and `@where` constraint names are data syntax belonging to their entry points, so they have no prefix. `if`, `then`, `else`, `block`, `param`, and result-declaration `result` use WAT syntax.

User-defined macros expand into typed expressions without changing standard WAT instructions or the reader. Struct construction `(@$T::{...})` is fixed compiler sugar, distinct from `@$name` user-macro calls. See [syntax, hygiene, and binding rules](user-macros.md).

In typed-core expression positions, `$foo` is shorthand for `local.get`, and numbers with a known expected scalar type are shorthand for `.const`. Let type annotations can use `(@let $x (type i32) 1 body)`. This `(type T)` is an annotation belonging to `@let`, not a standalone instruction. See [rules and examples](core-expressions.md#ローカル参照と数値の省略形).

Import basic types explicitly with `(@use "walz:core" (type $result) (type $option) (type $future) ...)`. Reference Result/Future as `(@inst $result T E)`/`(@inst $future T)` and non-parameterized text as `$string`. Multiple bindings initialize from left to right with `(@let (($a initial-a) ($b initial-b)) body)`. See [standard modules and bindings](standard.md) for detailed rules.

```wat
(module
  (func $id (@type.params $T) (@where (shared $T))
    (param $x $T) (result $T) (@effect pure)
    (local.get $x))
  (func $main (export "main") (result i64) (@effect pure)
    (@let $answer (call (@inst $id i64) (i64.const 42))
      (local.get $answer))))
```

`(result i32 i64)` is a standard multiple-result declaration. `(result (@inst $result i32 i64))` declares one Walz Result value.
Typed-core functions return zero values (unit) or one value. A pair is returned as one value and unpacked with pair-destructuring let. Standard WAT multi-value results are validated/compiled through the standard WAT path, without automatic conversion to typed-core multiple bindings.

```wat
(@use "./math.walz" (func $answer))  ;; ソースの静的リンク
(@export (func $answer))            ;; ソース API に公開
(import "host" "answer" (func $host-answer (result i32)))
(export "answer" (func $answer))    ;; Wasm API に公開
```

The [Type Imports proposal](https://github.com/WebAssembly/proposal-type-imports/blob/main/proposals/type-imports/Overview.md) form `(import "module" "name" (type ...))` is also WAT syntax, not file loading. Support for the proposal itself depends on the pinned `wat`/`wasmparser` versions. This change adds no type-import implementation.

<a id="標準-wat-との混在と解析"></a>

## Mixing Standard WAT and Analysis

Standalone standard WAT continues to validate and compile unchanged. The typed core applies its existing type, effect, ownership, and law checks.

WAT outside the typed core can be combined with syntax-only `@let`, stateful `@loop`/`@recur`, `@for`, and static name/type expansion. A Wasm validator checks the entire expanded module. Add an explicit type annotation to `@let` initializers whose type cannot be inferred.

This path returns `analysis.opaque=true`, conservatively all effects, and `ir=None`. It does not process extensions requiring checks, such as `@effect`, `@where`, owned values, closures, value types, tests, or laws. Component/Core+JSPI/verified generation requires the typed core.

`@let` and loop state are immutable and cannot be modified by standard `local.set`/`local.tee`. Numeric local indices refer only to original WAT parameters/locals and cannot reach generated temporaries. `@let` adds no control labels. `@recur` is a tail call to the innermost Walz loop, evaluating all arguments before updating state simultaneously.

## Input Rules

Walz extensions require `@`; a missing prefix is diagnosed as `E_WALZ_PREFIX`. Unknown entry points produce `E_WALZ_EXTENSION`. The language specification includes no migration command or compatibility with old syntax.

Only conversion for rerunning frozen evaluation inputs remains as an internal evaluation-harness operation. Historical inputs, results, and snapshots are preserved. After suite v7, which asked about standard-type imports and multiple bindings, evaluations including standard-API selection and an i64 counter change are distinguished as suite v8.

`just test-syntax` checks preservation of standard WAT, mixed execution, diagnostic positions, and checking boundaries.

See [typed-core expressions](core-expressions.md) for numeric instructions, return/labeled branches, flat Result chaining, and scalar newtypes.

## Match clauses

Walz match arms are `(@case variant ($bindings...) body)`. An optional final `(@default body)` covers all remaining variants, with no payload bindings. It must be unique and cover at least one variant. Every omitted payload must be shared data: owned and borrowed payloads need explicit `@case` handling. Type, effect, ownership, and runtime tag/subtype checks are retained. Default expansion is limited to 100,000 checked expression nodes across its generated arms; factor a large body into a function when needed. Standard Component WAT variant `(case "name" ...)` declarations remain unprefixed.

For a reversible alternate surface aimed at human review, see [optional Watz printing](review-syntax.md).
