# Optional Watz printing

Walz's primary source format is `.walz`, using WAT S-expressions. `walz print`
prints that form by default. `--format watz` selects an optional structural view
for human review, using `func`, `@export("name")`, immutable `let`, direct `match`/`if` expressions, and explicit Wasm/Walz operation names. It preserves the same source tree and adds no types
or runtime semantics.

Saved `.watz` views can also be read back and compiled through the same type,
effect, ownership, proof, and Wasm checks. Projects, core libraries, scaffolding,
Vite/TS7 integrations, and the playground continue to use canonical `.walz`.
The view has no JS execution, numeric coercion, implicit captures, Promises,
objects, arrays, or classes; it does not provide native TypeScript checking or
full LSP navigation.

## Commands

```sh
walz print examples/review/main.walz             # default: Walz S-expressions
walz print --format watz examples/review/main.walz > main.watz
walz print main.watz > restored.walz
walz check main.watz
walz run main.watz                    # 42
walz build main.watz                  # target/main.wasm
walz fmt main.watz                    # edits in place
walz fmt-check main.watz
```

`print` writes to stdout and does not type-check, expand macros, resolve imports,
or optimize the input. Build/check/run do the ordinary checks. The filename's
`.watz` suffix selects this parser; a `.walz` file still uses S-expressions.
Source modules can mix the two formats through explicit relative paths.
Entry discovery tries `main.walz`, `src/main.walz`, `main.watz`, then
`src/main.watz`. Test and benchmark discovery additionally accept
`*.test.watz` and `*_bench.watz`.

The file itself denotes one module. `module { ... }` is optional and the printer omits it. Bindings use `let`, remain immutable, and do not accept assignment or the `const` keyword.

## Example

The [canonical source](../examples/review/main.walz) and its
[generated view](../examples/review/main.watz) are checked together.

```watz
import { type $result } from "walz:core";

@effect(pure)
func $add($a: i64, $b: i64): i64 {
  return ($a + $b);
}

@export("main")
@effect(pure)
func $main(): i64 {
  let $sum: i64 = $add(40, 2),
      $answer: $result<i64, i32> = walz.result.ok($sum);
  return match ($answer) {
    case ok($value) => if<i64> ($value > 0) {
      $value
    } else {
      0
    },
    default => 0,
  };
}
```

`i32`, `i64`, `f32`, `f64`, and `v128` are Wasm types. A bare number requires
an expected scalar type, just as in canonical Walz. An omitted result annotation
means unit. `$` identifiers are retained so review does not rename symbols.

## Exact mapping

| Review form | Canonical form |
| --- | --- |
| Top-level declarations, optionally inside `module { ... }` | `(module ...)` |
| `import { type $T, $f as $g } from "./lib.walz";` | `(@use "./lib.walz" (type $T) (func $f $g))` |
| `export { type $T, $f };` | `(@export (type $T) (func $f))` |
| `@export("name")` | `(export "name")` on the following function |
| `@effect(reads, writes)` | `(@effect reads writes)` on the following function |
| `func $f($x: i64): i64 { return $x; }` | `(func $f (param $x i64) (result i64) $x)` |
| `let $x: i64 = 1; return $x;` | `(@let $x (type i64) 1 $x)` |
| `let $a = e1, $b = e2; return e3;` | `(@let (($a e1) ($b e2)) e3)` |
| `$f(a, b)` | `(call $f a b)` |
| `($a + $b)` | `(+ $a $b)` |
| `($a === $b)` / `($a !== $b)` | `(= $a $b)` / `(!= $a $b)` |
| `ref<$T>` / `refNull<$T>` in a type annotation | `(ref $T)` / `(ref null $T)` |
| `$List<i64>` in a type annotation | `(@inst $List i64)` |
| `wat.i64.add(a, b)` | `(i64.add a b)` |
| `match (value) { case ok($n) => $n, default => 0, }` | `(@match value (@case ok ($n) $n) (@default 0))` |
| `match<$T, R> (value) { ... }` | `(@match $T (result R) value ...)` |
| `match<$T> (value) { ... }` | `(@match $T value ...)` |
| `if<R> (test) { yes } else { no }` | `(if (result R) test (then yes) (else no))` |
| `if (test) { yes } else { no }` | `(if test (then yes) (else no))` |
| `walz.$macro(...)` | `(@$macro ...)` |
| `[$a, $b]` | `($a $b)` as a syntax operand list |

`@export("name")` on a `func` is its inline Wasm `(export "name")`
annotation. Source publication remains `export { ... };`, mapping to Walz's
`(@export ...)`. Neither form introduces implicit exports.

The final `return expression;` denotes the function's single expression body.
It does not insert a Wasm `return` instruction. An explicit canonical
`(return expression)` is written `wat.return(expression)`. There are no general
statement blocks: use `wat.block(...)` to sequence effects, direct `if`/`match`
for branches, and `walz.loop(...)`/`walz.for(...)` for loops.

Binary expressions must be fully parenthesized; the condition/subject parentheses
of `if` and `match` also serve this purpose. Extra grouping is allowed. There is
no precedence table or implicit conversion. Supported operators are the existing Walz operators:
`+ - * / === !== < <= > >= && || & | ^ << >> >>>`.
Their width, signedness, short-circuit behavior, and available type-class
instances remain those of [Walz operators](typeclasses.md).

Let initializers run once, left to right. Several declarations encode nested
lets; comma-separated declarations encode one grouped let. A shadowing let stays an explicit `walz.let(...)` so a JS temporal dead zone
is not suggested. All bodies still end with a single expression.

## Match and if expressions

Match uses `case variant($bindings...) => expression` and optional
`default => expression` clauses, separated by commas. A trailing comma is
allowed. Watz uses bare `case` and `default`; lowering restores the canonical
`@case` and `@default` heads. Empty payloads retain `()`. Payload names are scoped to their own arm;
there are no guards, nested patterns, implicit captures, or wildcard moves.
The ordinary checker enforces variant types, payload arity, exhaustiveness,
unique cases, and a unique final nonredundant default. A default cannot discard
owned or borrowed payloads. The subject runs once and only the selected body runs.

`match (subject)` retains the canonical inferred form, using the surrounding
expected result type. `match<$Variant, R> (subject)` retains an explicit variant
and `(result R)`; `match<$Variant> (subject)` retains an omitted result declaration.
These are source annotations, not runtime generic dispatch.

Canonical `.walz` also accepts `$List<i64>` and nested angle type arguments as
shorthand for `@inst`; see [application rules](generics.md#angle-application-shorthand).

Value-producing if expressions use `if<R> (condition) { value } else { value }`.
`<R>` maps exactly to `(result R)`; omitting it retains the canonical omitted
result declaration, which means unit. There is no new result-type inference.
`if<>` preserves an explicit empty result declaration; native multi-value
headers use `if<i32, i64>`. A condition must be i32, with Wasm's zero/nonzero rule.
Neither JS truthiness nor numeric conversion is introduced.

`else if` represents an if expression as the sole expression in the outer
else branch. Branch braces contain one expression (or an empty native branch),
without an implicit `block` or `return`. A trailing semicolon inside these braces
is rejected. Use `walz.let(...)` for a let expression inside a branch and
`wat.block(...)` for sequencing. Omitted else branches remain subject to the
underlying Wasm/typed-core rules; ordinary typed-core if expressions have both
branches.

Unusual native forms retain structural spelling: labeled or stack-style if,
multi-instruction branch bodies, and match headers with an explicit empty
`(result)` declaration. This preserves the source tree instead of inferring a
different shape. Bare data symbols named `if` or `match` use `sexpr.atom(...)`.

## Forms without dedicated sugar

Every remaining source form has an explicit structural spelling. Prefix native
forms with `wat.` and extensions with `walz.`. These are syntax markers, not
objects, namespaces, or runtime property lookups. The rest of the instruction
name is preserved: `wat.i32.shr_u(...)` keeps its unsigned Wasm meaning.

Structural spelling is also accepted for control expressions, for example:

```watz
wat.if(wat.result(i64), condition, wat.then(yes), wat.else(no))
```

Generic function/closure declarations, noncanonical headers, stack-style WAT,
multi-value functions, host imports, named fields, and unusual identifiers use
this structural spelling rather than adding another source-level meaning. For
example, explicit function instantiation is
`wat.call(walz.inst($id, i64), wat.i64.const(42))`.
`wat.import(...)` retains a host import, distinct from source `import { ... }`.

The complete escape forms are:

- `sexpr.atom("$name-with-hyphens")`: one atom, preserving its spelling.
- `sexpr.form("@name-with-hyphens", ...operands)`: a form with an arbitrary atom head.
- `sexpr.bytes("ff00")`: raw bytes for non-UTF-8 WAT strings.
- `[...operands]`: a syntax list with no selected instruction head.

The `...` above is documentation notation; there is no spread operator.
String literals use JSON escaping. They become WAT strings with the same text.
Reserved bare atoms starting with `wat.` or `walz.` are escaped to avoid changing
their identity. Func/let sugar requires simple `$name` identifiers;
other names are preserved with structural forms, without automatic renaming.

## Round trips and locations

Round trips preserve the exact atom/text/byte/list tree before macro expansion.
Formatting and comment placement are not byte-preserved; an optional module wrapper is normalized away. Original comments are
copied into the generated view's preamble with source line numbers; review
comments are retained as canonical `;; @review-note` comments. Formatting this
view again is stable.

Lowering records source mappings. CLI checker errors point into the original
`.watz` file, including when that file is an imported dependency. The public
Rust API is `walz::review::{to_watz, from_watz}`; ordinary
`walz::compile` still accepts canonical S-expression strings. Filesystem APIs
under `walz::source` select the parser from the filename.

Input size, tokens, lexical/canonical nesting, and binding depth are bounded.
Malformed review syntax reports `E_REVIEW_SYNTAX`; semantic diagnostics keep the
ordinary checker codes. `just test-print` checks corpus round trips, mixed
modules, CLI runners, diagnostic locations, and match behavior.
