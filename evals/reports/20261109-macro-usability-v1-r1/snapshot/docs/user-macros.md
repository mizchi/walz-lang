# User-defined syntax

WLT provides declarative, hygienic expression macros. A macro consumes S-expression syntax and expands into existing checked WLT. The reader, standard WAT instructions, type system, effect rules and ownership rules remain compiler-defined. Macro expansion does not run guest code, invoke host imports, or expose file/network/time/random APIs.

Run the complete example:

```sh
wlt test examples/user-macros/main.wlt
wlt run examples/user-macros/main.wlt
wlt macro-expand examples/user-macros/main.wlt
```

The last command emits JSON with the `wlt-macro-expansion-v1` schema. `wlt expand` continues to emit final standard WAT.

## Patterns and templates

```wat
(module
  (%macro $twice
    (rule (?value:expr)
      (%let $saved ?value (+ $saved $saved))))

  (func $main (export "main") (result i32)
    (%effect pure)
    (%$twice (i32.const 21))))
```

`%macro` is a module-level declaration. `%$twice` calls the macro named `$twice`; `%let` and the other built-in forms retain their existing meanings. Macro bindings occupy a separate namespace from functions and types. Standard WAT heads cannot be redefined.

A rule has an argument-list pattern, optional clauses, and one output expression. Rules are tried in declaration order. Pattern variables use `?name:category`; templates refer to them as `?name`. The categories are:

| Category | Captured syntax |
| --- | --- |
| `expr` | An expression to elaborate in its eventual context |
| `ident` | A `$name` identifier |
| `type` | Type syntax; names resolve in the caller's module |
| `literal` | A literal token or string |

`expr` is a syntax category, not a claim that the expression already has a checked value type. Non-variable pattern elements match literal syntax. Patterns see normalized WLT: built-in extension heads have their core names, global operands are resolved, and existing generic applications may already be specialized. Use `expr` and `type` captures to pass existing code through; macro-specific keywords and grouping can define a DSL grammar. This interface does not expose raw source-token rewriting. A trailing `pattern ...` matches zero or more elements; one repetition level is supported. Templates use the same trailing notation. Repeated captures used together must have equal lengths.

```wat
(module
  (%macro $sum
    (rule (?value:expr) ?value)
    (rule (?value:expr ?rest:expr ...)
      (+ ?value (%$sum ?rest ...))))
  (func $main (export "main") (result i32)
    (%effect pure)
    (%$sum (i32.const 10) (i32.const 12) (i32.const 20))))
```

Expansion may recursively call macros, within the compiler's limits. It cannot declare macros dynamically or change the enclosing function's contracts. Outputs must belong to the checked WLT expression subset; arbitrary native stack-WAT macro expansion is outside this interface.

## Inferred types and rule selection

An `infer` clause asks the ordinary checker for the type of an expression in the current lexical context. The inferred type becomes syntax that the template can use in annotations, constructors and generic applications.

```wat
(module
  (func $id (%type.params $T) (param $value $T) (result $T)
    (%effect pure)
    $value)
  (%macro $identity
    (rule (?value:expr)
      (infer (?T ?value))
      (call (%inst $id ?T) ?value)))
  (func $main (export "main") (result i64)
    (%effect pure)
    (%$identity (i64.const 42))))
```

`?T` here is the compiler's concrete type, including a nominal GC type or a constructed list/result/etc. Generic applications generated from it pass through ordinary monomorphization and generic constraints. Calls inside a generic function are expanded separately in each concrete specialization.

The supported queries are:

```text
(infer (?T ?expression))           ;; Infer without a result-type hint.
(infer (?T expected))              ;; Read this invocation's expected result type.
(infer (?T ?expression expected))  ;; Check the expression with that explicit hint.
```

There is no default numeric width. The first query rejects a bare `1` whose type is ambiguous; use an explicit constant, an annotation, or the explicit `expected` hint. A query also fails when no expected type is available, or when it would have to reify the bottom type. Macro-generated binders do not exist while querying an unexpanded argument body; use `expected` or query the initializer instead.

`when` clauses select a rule using a concrete inferred type or an existing sealed scalar class:

```wat
(module
  (%macro $width
    (rule (?value:expr) (when (type ?value i32)) (i32.const 32))
    (rule (?value:expr) (when (type ?value i64)) (i32.const 64)))
  (%macro $twice
    (rule (?value:expr)
      (when (Add ?value))
      (%let $saved ?value (+ $saved $saved))))
  (func $main (export "main") (result i32)
    (%effect pure)
    (+ (%$width (i64.const 0)) (%$twice (i32.const -11)))))
```

Available class predicates are `Add`, `Sub`, `Mul`, `Div`, `Eq`, `Ord`, `Logical`, `Bitwise` and `Shift`. A false predicate tries the next rule. An ill-typed expression reports its own error; it is not treated as a failed predicate. Queries perform no runtime evaluation. They do not grant the macro access to compiler IR, mutable inference state, a type-check bypass, or arbitrary compiler code.

## Hygiene, evaluation and intentional bindings

Template-introduced locals receive fresh identities. Captured expressions retain their caller's lexical context. Function/type references written in a template resolve at its definition site, so an exported macro may call its module's private helper without accidentally calling a same-named function in the caller.

Hygiene does not guarantee a single evaluation. `(+ ?value ?value)` duplicates its argument. The `%let` in `$twice` above evaluates its argument once. Effects and ownership are checked on the actual expansion: a duplicated move is rejected, and a host call still needs its host effect. Discarded syntax is not runtime code and is not value-type-checked unless an inference query requests it. Declaring an `expr` fragment alone does not change that rule.

To introduce a caller-supplied name, declare its scope with `bind`:

```wat
(module
  (%macro $with
    (rule (?name:ident ?value:expr ?body:expr)
      (bind ?name ?body)
      (%let ?name ?value ?body)))
  (func $main (export "main") (result i32)
    (%effect pure)
    (%let $x (type i32) 40
      (%$with $x (+ $x 1) (+ $x 1)))))
```

Only `?body` can see the new binding; `?value` sees the outer `$x`. The expander checks that the template actually introduces the declared binder and places the body in its lexical scope. Caller-supplied binders must be single identifiers introduced once; multiple bind clauses may bind distinct names for a body. There is no implicit name capture or unrestricted hygiene escape.

## Static modules

Macros use the same explicit filesystem module boundaries as other WLT declarations:

```text
;; Library
(%export (macro $twice))

;; Caller
(%use "./syntax.wlt" (macro $twice $double))
(%$double (i32.const 21))
```

Private macros cannot be selected. Aliases and explicit re-exports work. Macro declarations disappear before executable IR; ordinary DCE applies to generated calls and their helpers. Macro bodies cannot generate new filesystem imports during checking. Libraries retain their normal opaque-type boundaries.

## Diagnostics and editor support

Syntax nodes carry snapshot-local identities and expansion context in addition to source ranges. Captured input preserves its source location; generated syntax records its template location and enclosing expansion. The public report contains definitions, selected rules, inferred type syntax, output in normalized WLT syntax, and per-node origins. Each captured input records its output node IDs: an empty list identifies discarded syntax, while several IDs identify duplication. Its raw spans address the loaded immutable compilation unit; CLI output additionally includes original-file locations for calls and definitions. Normalized output is an inspection format, not a source-file roundtrip API.

LSP provides macro signatures, definitions, ordinary checked argument types, and completion/definition for caller-supplied binders. The macro head displays its syntax signature; it does not receive the type of a generated operand. Discarded arguments likewise do not inherit the expansion's result type. On an invalid current revision, binding information from the declared grammar remains available and is explicitly unverified; old inferred types are not reused. If matching rules disagree about a body's binding structure before type checking, recovery does not guess a scope.

The custom `wlt/expandMacro` request takes the same `textDocument` and `position` parameters as hover and returns the report entries covering that position. Successful checked snapshots supply this view; an invalid revision does not publish an old expansion as current. Initialization advertises `experimental.wltMacroExpansion`.

Current limits are 512 expansions, 64 nested expansion contexts, 100,000 generated origin nodes, a 256-level syntax nesting limit, a 4 MiB template expansion budget, 8 MiB of recorded expansion text, and 2,000,000 syntax-node visits across elaboration restarts. Existing checker, alias and monomorphization limits also apply.

## Design references

The binding declarations follow the separation of syntax and binding specifications in [Racket syntax-spec](https://docs.racket-lang.org/syntax-spec-v3/Specifying_languages.html). Hygiene and the separation of syntax transformation from elaboration are informed by [Lean's macro system](https://lean-lang.org/doc/reference/latest/Notations-and-Macros/Macros/). Input and expansion identities are retained for editor support, as illustrated by [Racket Check Syntax](https://docs.racket-lang.org/tools/Check_Syntax.html) and [rust-analyzer expansion mapping](https://rust-lang.github.io/rust-analyzer/hir_expand/struct.ExpansionInfo.html).
