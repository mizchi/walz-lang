# User-defined syntax

Walz provides declarative, hygienic expression macros. A macro consumes S-expression syntax and expands into existing checked Walz. The reader, standard WAT instructions, type system, effect rules and ownership rules remain compiler-defined. Macro expansion does not run guest code, invoke host imports, or expose file/network/time/random APIs.

Use an ordinary function to abstract values. Ordinary generic calls already infer unambiguous type arguments, so a macro that only supplies `@inst` is usually unnecessary. Use a macro when the abstraction needs syntax, a declared binding scope, or type-directed expansion. For example, `call $option.ok-or` can infer its arguments without a macro; `@$with` below introduces a caller-named local into a body.

Run the complete example:

```sh
walz test examples/user-macros/main.walz
walz run examples/user-macros/main.walz
walz macro-expand examples/user-macros/main.walz
walz macro-expand --text examples/user-macros/main.walz
```

`walz macro-expand` emits JSON with the `walz-macro-expansion-v1` schema; `--text` prints a readable expansion history. `walz expand` continues to emit final standard WAT. `macro-expand` and `expand` check the program without running the guest.

## Patterns and templates

```wat
(module
  (@macro $twice
    (rule (?value:expr)
      (@let $saved ?value (+ $saved $saved))))

  (func $main (export "main") (result i32)
    (@effect pure)
    (@$twice (i32.const 21))))
```

`@macro` is a module-level declaration. `@$twice` calls the macro named `$twice`; `@let` and the other built-in forms retain their existing meanings. Macro bindings occupy a separate namespace from functions and types. Standard WAT heads cannot be redefined.

A rule has an argument-list pattern, optional clauses, and one output expression. Rules are tried in declaration order. Pattern variables use `?name:category`; templates refer to them as `?name`. The categories are:

| Category | Captured syntax |
| --- | --- |
| `expr` | An expression to elaborate in its eventual context |
| `ident` | A `$name` identifier |
| `type` | Type syntax; names resolve in the caller's module |
| `literal` | A literal token or string |

`expr` is a syntax category, not a claim that the expression already has a checked value type. Non-variable pattern elements match literal syntax. Patterns see normalized Walz: built-in extension heads have their core names, global operands are resolved, and existing generic applications may already be specialized. Use `expr` and `type` captures to pass existing code through; macro-specific keywords and grouping can define a DSL grammar. This interface does not expose raw source-token rewriting. A trailing `pattern ...` matches zero or more elements; one repetition level is supported. Templates use the same trailing notation. Repeated captures used together must have equal lengths. Grouping such as `(($name expression) ...)` is macro syntax; every contained expression retains ordinary Walz name and type resolution.

```wat
(module
  (@macro $sum
    (rule (?value:expr) ?value)
    (rule (?value:expr ?rest:expr ...)
      (+ ?value (@$sum ?rest ...))))
  (func $main (export "main") (result i32)
    (@effect pure)
    (@$sum (i32.const 10) (i32.const 12) (i32.const 20))))
```

Expansion may recursively call macros, within the compiler's limits. It cannot declare macros dynamically or change the enclosing function's contracts. Outputs must belong to the checked Walz expression subset; arbitrary native stack-WAT macro expansion is outside this interface.

## Inferred types and rule selection

An `infer` clause asks the ordinary checker for the type of an expression in the current lexical context. The inferred type becomes syntax that the template can use in annotations, constructors and generic applications.

```wat
(module
  (func $id (@type.params $T) (param $value $T) (result $T)
    (@effect pure)
    $value)
  (@macro $identity
    (rule (?value:expr)
      (infer (?T ?value))
      (call (@inst $id ?T) ?value)))
  (func $main (export "main") (result i64)
    (@effect pure)
    (@$identity (i64.const 42))))
```

`?T` here is the compiler's concrete type, including a nominal GC type or a constructed list/result/etc. Generic applications generated from it pass through ordinary monomorphization and generic constraints. Calls inside a generic function are expanded separately in each concrete specialization.

The supported queries are:

```text
(infer (?T ?expression))           ;; Infer without a result-type hint.
(infer (?T expected))              ;; Read this invocation's expected result type.
(infer (?T ?expression expected))  ;; Check the expression with that explicit hint.
```

There is no default numeric width. The first query rejects a bare `1` whose type is ambiguous; use an explicit constant, an annotation, or the explicit `expected` hint. A query also fails when no expected type is available, or when it would have to reify the bottom type. Macro-generated binders do not exist while querying an unexpanded argument body; use `expected` or query the initializer instead.

### Read-only structural queries

Use a selector to inspect the checked type rather than matching the compiler's normalized type spelling:

```wat
(@macro $unwrap
  (rule (?value:expr)
    (infer (?T (option.element ?value)))
    (@match (@inst $option ?T) (result ?T) ?value
      (@case none () (unreachable))
      (@case some ($value) $value))))
```

The defining module explicitly imports `$option` from `walz:core`. Selectors are `option.element`, `list.element`, `result.ok`, `result.err`, `tuple.first`, `tuple.second`, `box.element`, `future.element`, `stream.element`, `task.element`, `array.element` and `ref.heap`. Each takes one captured, non-repeated expression. A selector is checked against its constructor; a mismatch reports `E_MACRO_TYPE`. `ref.heap` returns the nominal heap name of either a nullable or non-null reference. `array.element` requires an array reference and respects the defining module's opaque representation boundary, including generic specializations. Queries never run the expression or mutate its inferred type.

Reports advertise `type_schema: "walz-type-info-v1"`. Each expansion's `types` map contains structured `TypeInfo` values, using a `kind` tag and named children (`element`, `ok`/`err`, `first`/`second`). Public constructor tags include `string`, `tuple` and `box`; they do not use internal `text`, `pair` or `own` spellings. Scalar types have `kind: "scalar"` and `name: "i64"` etc. References expose `name` and `nullable`, without private layout. Nominal names are identities within the checked snapshot; generated suffixes are not a stable naming API. The older `inferred` text map remains an inspection view. The Rust API exposes the same read-only `macros::TypeInfo` enum.

`when` clauses select a rule using a concrete inferred type or an existing sealed scalar class:

```wat
(module
  (@macro $width
    (rule (?value:expr) (when (type ?value i32)) (i32.const 32))
    (rule (?value:expr) (when (type ?value i64)) (i32.const 64)))
  (@macro $twice
    (rule (?value:expr)
      (when (Add ?value))
      (@let $saved ?value (+ $saved $saved))))
  (func $main (export "main") (result i32)
    (@effect pure)
    (+ (@$width (i64.const 0)) (@$twice (i32.const -11)))))
```

Available class predicates are `Add`, `Sub`, `Mul`, `Div`, `Eq`, `Ord`, `Logical`, `Bitwise` and `Shift`. A false predicate tries the next rule. An ill-typed expression reports its own error; it is not treated as a failed predicate. Queries perform no runtime evaluation. They do not grant the macro access to compiler IR, mutable inference state, a type-check bypass, or arbitrary compiler code.

## Hygiene, evaluation and intentional bindings

Template-introduced locals receive fresh identities. Captured expressions retain their caller's lexical context. Function/type references written in a template resolve at its definition site, so an exported macro may call its module's private helper without accidentally calling a same-named function in the caller.

Hygiene does not guarantee a single evaluation. `(+ ?value ?value)` duplicates its argument. The `@let` in `$twice` above evaluates its argument once. Effects and ownership are checked on the actual expansion: a duplicated move is rejected, and a host call still needs its host effect. Discarded syntax is not runtime code and is not value-type-checked unless an inference query requests it. Declaring an `expr` fragment alone does not change that rule.

To introduce a caller-supplied name, declare its scope with `bind`:

```wat
(module
  (@macro $with
    (rule (?name:ident ?value:expr ?body:expr)
      (bind ?name ?body)
      (@let ?name ?value ?body)))
  (func $main (export "main") (result i32)
    (@effect pure)
    (@let $x (type i32) 40
      (@$with $x (+ $x 1) (+ $x 1)))))
```

Only `?body` can see the new binding; `?value` sees the outer `$x`. The expander checks that the template actually introduces the declared binder and places the body in its lexical scope. Each caller-supplied identifier must be introduced once. The binder fragment may be a single `ident` or a trailing repeated `ident`; the body must be one non-repeated `expr`. Duplicate caller names for the same body are rejected. Multiple bind clauses may bind distinct names for a body. There is no implicit name capture or unrestricted hygiene escape.

### Repeated bindings and delegation

```wat
(@macro $bindings
  (rule (((?name:ident ?value:expr) ...) ?body:expr)
    (bind ?name ?body)
    (@let ((?name (type i64) ?value) ...) ?body)))
(@macro $with-values
  (rule (((?name:ident ?value:expr) ...) ?body:expr)
    (bind ?name ?body)
    (@$bindings ((?name ?value) ...) ?body)))

(@$with-values (($a (i64.const 20)) ($b (i64.const 22))) (+ $a $b))
```

The template keeps its binding group until repetition is expanded, then lowers it into ordinary nested lets. Empty repetitions evaluate the body without adding bindings. Groups are limited to 256 bindings, with the existing nesting and expansion budgets also enforced. Initializers run left to right in the caller's lexical context: `(bind ?name ?body)` makes the new names visible only to `?body`. A captured initializer does not implicitly gain access to an earlier generated binding. Ordinary Walz let groups keep their existing sequential scope.

Delegation checks the callee's declared binding interface before substitution. Every syntactically matching callee rule must agree on its binder/body inputs, even when type guards differ. The enclosing rule must advertise the same `bind` relationship. Forward the captured expression as an entire body argument; arbitrary reconstructed binding scopes are not inferred. The callee still verifies that it actually introduces the binders in the promised scope. Forwarding cannot expose a name to the initializer or to a second occurrence outside that body position. This composes across multiple layers and repeated binder groups.

| Task | Current boundary | Practical choice |
| --- | --- | --- |
| A later initializer uses an earlier caller-supplied binder | Captured initializers retain the caller's scope | Compute dependencies with an ordinary outer `@let`, then pass them in |
| Forward a body while wrapping it in a block | Only the whole captured body is forwarded | Put the wrapper inside the callee's binding scope |
| Repeat bindings before a final Result body | A template repetition must end its list | Use built-in `@result.do` for sequencing; a recursive user DSL needs separate design and evaluation |

These limits do not change the sequential scope of ordinary grouped `@let`. A rejected direct Result template is not evidence that every recursive macro definition is impossible.

Run the complete [binding example](../examples/user-macros/bindings.walz) with `walz test examples/user-macros/bindings.walz`.

## Static modules

Macros use the same explicit filesystem module boundaries as other Walz declarations:

```text
;; Library
(@export (macro $twice))

;; Caller
(@use "./syntax.walz" (macro $twice $double))
(@$double (i32.const 21))
```

Private macros cannot be selected. Aliases and explicit re-exports work. Macro declarations disappear before executable IR; ordinary DCE applies to generated calls and their helpers. Macro bodies cannot generate new filesystem imports during checking. Libraries retain their normal opaque-type boundaries.

## Diagnostics and editor support

Syntax nodes carry snapshot-local identities and expansion context in addition to source ranges. Captured input preserves its source location; generated syntax records its template location and enclosing expansion. The public report contains definitions, selected rules, inferred type syntax, output in normalized Walz syntax, and per-node origins. Each captured input records its output node IDs and `usage`: `discarded` (zero), `single` (one), or `duplicated` (several). These count syntax occurrences in that expansion, not runtime evaluations. A single occurrence inside a loop may run repeatedly; a branch may never execute. `walz macro-expand --text` displays the counts, input locations, inferred types and expanded syntax; the default command retains JSON. Its raw spans address the loaded immutable compilation unit; CLI output additionally includes original-file locations for calls and definitions. Normalized output is an inspection format, not a source-file roundtrip API.

LSP provides macro signatures, definitions, ordinary checked argument types, and completion/definition for caller-supplied binders. The macro head displays its syntax signature; it does not receive the type of a generated operand. Discarded arguments likewise do not inherit the expansion's result type. On an invalid current revision, binding information from the declared grammar remains available and is explicitly unverified; old inferred types are not reused. For missing closing parentheses, the editor reads current imports and unsaved library declarations through a separate recovery snapshot. It appends only closing delimiters, preserves original token positions, and never compiles that snapshot or publishes checked types/expansions from it. Strings, nested comments and extra closing parentheses do not create fabricated tokens. Import aliases, macro definitions and repeated binder completions use the current dependency contents. If matching rules disagree about a body's binding structure before type checking, recovery does not guess a scope.

An ellipsis before the end of a template list is diagnosed at the `...` token, with a suggestion to use trailing repetition or recursive rules. A reconstructed delegated body receives a suggestion to forward the body unchanged and put the wrapper inside the callee's scope. If an initializer refers to a caller-supplied binder outside its declared body, the diagnostic explains the caller's scope and suggests an ordinary outer `@let` or an explicit dependency. This hint requires a matching captured binder; unrelated unknown locals keep their ordinary diagnostics.

The custom `walz/expandMacro` request takes the same `textDocument` and `position` parameters as hover and returns the report entries covering that position. Successful checked snapshots supply this view; an invalid revision does not publish an old expansion as current. Initialization advertises `experimental.walzMacroExpansion`.

### Checking scope and work counters

Ordinary syntax expansion resumes at the affected expression against its existing immutable type/function/heap environment. Checked parents and earlier sibling expressions remain on the checker stack. A speculative type query defers expansion to its containing expression: temporary lexical IDs are rolled back, and only expanded syntax is replayed. No checked IR from a probe is reused. The completed function receives a final expanded-syntax size/depth check, and all module-level type, effect, ownership, resource, interface, race and benchmark obligations are still validated before accepting the module. A new generic specialization invalidates that environment and triggers a whole-module pass. There is no cross-revision reuse of checked types.

The report's `checking` object exposes `module_passes`, `function_checks` and budgeted `syntax_node_visits`. Function checks count entries into a function body, rather than expression retries. Node visits charge input traversal, resumed syntax and final expanded-body validation; they do not count every checker operation. These are deterministic work counters, not CPU time or runtime execution counts. The existing 2,000,000-node work limit has not been raised. Both 512 independent macro-bearing functions and 512 sibling expansions in one function are regression-tested with one module pass and one function check per defined body. Complex speculative expressions can still retry their own contents.

Current limits are 512 expansions, 64 nested expansion contexts, 100,000 generated origin nodes, a 256-level syntax nesting limit, a 4 MiB template expansion budget, 8 MiB of recorded expansion text, and 2,000,000 syntax-node visits across elaboration restarts. Existing checker, alias and monomorphization limits also apply.

## Differential tests and shrinking

```sh
just test-macro-tools
just test-macro-composition
just fuzz-user-macros 42 64
just fuzz-user-macros-replay target/macro-fuzz/failures/<run>/reduced.json
```

The seeded corpus compares user macro calls, independently rendered manual expansions and a Rust value/trace model. It exercises hygiene, repeated captures, single/duplicate/discarded inputs, wrapping i64 arithmetic, branches, caught exceptions, owned boxes, generic calls and delegated/repeated binder scopes (including caller-name collisions). Both optimized and unoptimized Wasm run in Wasmtime and V8; effects are compared, and negative programs compare type/effect/exception/ownership diagnostics. Failures preserve their classification during structural shrinking. Original and reduced JSON ASTs, macro/manual Walz, seed, case and diagnostics are written under ignored `target/macro-fuzz/failures/`. These are regression evidence, not a proof of all macro transformations.

Application, editor, authoring and scale evaluations use fresh run IDs. See the [evaluation guide](../evals/README.md#マクロの実用性と規模の評価) for the separate v1/v2 tasks and [the expression-resumption comparison](../evals/reports/20261109-macro-authoring-v2-r3/REPORT.md) for fixed inputs, remaining limitations and measured compiler costs.

## Design references

The binding declarations follow the separation of syntax and binding specifications in [Racket syntax-spec](https://docs.racket-lang.org/syntax-spec-v3/Specifying_languages.html). Hygiene and the separation of syntax transformation from elaboration are informed by [Lean's macro system](https://lean-lang.org/doc/reference/latest/Notations-and-Macros/Macros/). Input and expansion identities are retained for editor support, as illustrated by [Racket Check Syntax](https://docs.racket-lang.org/tools/Check_Syntax.html) and [rust-analyzer expansion mapping](https://rust-lang.github.io/rust-analyzer/hir_expand/struct.ExpansionInfo.html).
