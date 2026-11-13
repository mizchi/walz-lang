<a id="walz-v0--wat-の基底と型付き拡張"></a>

# walz v0 — WAT Base and Typed Extensions

[English](SPEC.md) | [日本語](SPEC.ja.md)

<a id="標準-wat-との互換性"></a>

## Compatibility with Standard WAT

Inputs that parse and validate as standard WAT take precedence and are compiled to Wasm without source changes.
Support is defined by the default validation features of the pinned `wat`/`wasmparser` versions. Stack syntax, anonymous functions, numeric indices, linear memory, byte strings, standard exception instructions, and similar features are not rejected because of typed-core restrictions.
If the original WAT reassigns locals or makes dynamic calls, its Wasm semantics are preserved. The `$__` reservation is also a convention of the extension path only.

Every Walz extension entry point starts with `@`. Standard `import` / `export` are distinct from static-linking `@use` / `@export`. The value type `(@inst $result T E)` is also distinct from the result declaration `(result T)`. Type arguments are applied with `(@inst $name T...)`. [Prefixes and analysis boundaries](docs/syntax.md) describes these rules.

Known Walz `@` names are reserved and always enter checking. Other WAT annotations and their nested payloads remain opaque; extension-looking forms inside metadata are not executed. WIT identifier escapes and URI percent encoding retain `%`.
Local-reference and numeric expression shorthands and the fixed symbolic operators have no prefix. In addition to arithmetic and comparison, Walz provides `&&`, `||`, `&`, `|`, `^`, `<<`, `>>`, and `>>>`.

Additional syntax is checked for names, types, bindings, and exhaustiveness in the typed core described below, then expanded to standard WAT. Bodies in this core follow the folded-expression convention. When WAT outside the core is combined with syntax macros `@let` / `@loop` / `@for`, the entire expanded Wasm module is validated and analysis is marked opaque. Effect, ownership, and similar contracts are never considered checked through this path.
Expansion rules for built-in syntax are fixed. Users can define syntax extensions as declarative expression macros; neither eval nor arbitrary compile-time programs are executed.
The `expand`/`wat` commands and `walz::expand` expose the expansion output.

User-defined expression syntax is declared with `@macro` and invoked with `@$name`. It provides declarative patterns/templates, syntax categories, hygienic bindings, explicit `bind` scopes, `infer` queries to the ordinary type checker, and `when` constraints. Output returns to existing typed Walz and passes monomorphization, type, effect, ownership, and race checks. Macros have no API for registering new typing rules or checked IR. Original source positions and expansion contexts are retained, and the LSP uses the declared binding rules too.

`bind` can also be declared for repeated identifiers, and an entire body can be delegated to a macro that declares the same binding relationship. Captured initializer expressions are interpreted in the caller's scope; new bindings are visible only in the declared body. After ordinary expansion, checking resumes from the affected expression, retaining checked enclosing and preceding expressions. Temporary locals created by inference attempts are not carried over; if new instantiation changes the type environment, the entire input is checked again. Syntax checks on function completion and a final whole-module contract check are also performed. See [User-defined macros](docs/user-macros.md) for syntax, limits, and public APIs.

`walz::validate` and CLI `check` support both paths. `walz::check` is the typed-core API for obtaining detailed types.
The `ir` field of `compile` is `Option<Module>` and is `None` when standard WAT falls outside the core.
Wasm static validation still runs in that case, and `native::types` and the `types` command expose function parameters, multiple results, and exports. Per-expression positions are not provided.
Inputs that cannot be analyzed in detail return `analysis.opaque=true` and true for every `module_effects` field. An empty function list must not be interpreted as “no effects.”
For the typed core, `module_effects` is the union of effects of all functions; it does not describe the effects of module instantiation itself.

The following type, immutability, and syntax rules apply to typed extensions.

<a id="表現と型"></a>

## Representation and Types

Walz's basic data types are imported explicitly with `(@use "walz:core" (type $result) (type $option) (type $future) ...)`. Type application uses the same syntax as user-defined generic types, such as `(@inst $result T E)` and `(@inst $future T)`. `$string`, `$bytes`, and `$unit` are aliases with no type arguments. Wasm numeric types and `ref` need no imports. See [Standard module](docs/standard.md).

A program is one `(module ...)`. Declarations include WAT-style `type`, `func`, function `import`, and `tag`, plus `@closure`, `@test`, `@bench`, `@law`, and static named `@use` / `@export`. `@include` is also allowed at declaration positions in a file.
Types are scalar-newtype `$Name`, `i32`, `i64`, `f32`, `f64`, `v128`, `(ref $T)`, `(ref null $T)`, `(@inst $option T)`, `(@inst $result T E)`, `(@inst $tuple T U)`, `(@inst $box T)`, `(@inst $future T)`, `(@inst $stream T)`, `$bytes`, and `$string`. Expressions and functions with no declared result have type unit.

The typed core supports `v128` and non-memory instructions from standard SIMD. The instruction specifies the lane shape; all use the same `v128` type. Immediates and operands are checked and emitted directly as Wasm instructions. A vector is itself a shared/nonunit value and can be stored in GC data, captured, or used in monomorphization. See [SIMD](docs/simd.md) for supported operations and examples.
Function parameters/results, struct fields, array elements, and closure captures require types.
Without an annotation, a `@let` binding's type is determined uniquely from its initializer; `case` binding types come from payload declarations.
The `(type T)` in `(@let $x (type i32) 1 body)` is an explicit let annotation, also available for multiple bindings. `$foo` in expression position means `(local.get $foo)`, and bare numbers expand to `.const` of the expected scalar type. Numbers without an expected type have no default width and cause a type error. Moves of owned values, closure captures, and newtype wrapping are explicit. See [Expression shorthand rules](docs/core-expressions.md#ローカル参照と数値の省略形).
There are no implicit type conversions; only a non-null reference can conform to a nullable reference of the same type.
GC types are distinguished by name. Forward references and recursive types are allowed and are emitted together in one Wasm recursion group.

Numeric semantics are the same as WAT. Integer arithmetic wraps at the bit width; instruction names distinguish signed and unsigned interpretations.
Integer division and remainder trap under Wasm's conditions. Floating-point operations also follow Wasm semantics.
Scalar conversions, saturating conversions, rounding, sqrt, min/max, and reinterpretation are handled directly in the typed core too. [Numeric and control rules](docs/core-expressions.md) contains examples.
The `numeric_ops!` macro in `src/ir.rs` defines the numeric instruction list and type constraints for the typed core. The instruction set of the standard-WAT path is not limited to this table.

Name resolution, static typing, effect contracts, and ownership checks can be run independently through `walz::check(&str)`, which returns checked typed IR. This API does not prove laws.
This stage checks function parameters/results, calls, numeric instructions, GC data construction/reads/writes, enum decomposition, closure captures, and loop state updates.
Type inference is monomorphic inference based on declared types. Built-in value-type constructors and explicitly declared user-defined generic functions/types are instantiated for each set of type arguments. Ordinary generic function `call` expressions can omit type arguments when arguments, the expected type, and known callable signatures determine them uniquely. There is no whole-program inference of function signatures or implicit numeric conversion. See [Monomorphization contract](docs/generics.md).
A function without a result annotation, or a block/if/match without result, has type unit; omission does not request inference of a result type from its body.

`typing::report(&module)` returns per-function parameter/capture IDs and result types; IDs, names, declaration positions, types, and binding kinds for all bindings; and positions/types for every source expression.
Even bindings with the same name are distinguished by IDs within a function. Type reports use underlying notation with aliases expanded, such as `i64`, `(ref $T)`, `(ref null $T)`, `@unit`, and `never`. Standard types in source are referred to by their imported `walz:core` names.
Positions are half-open UTF-8 byte-offset ranges `[start, end)` in the source passed to the API (after expansion when include is used). Omitted unit bodies have no source-expression position and are excluded from the expression list.
`typing::type_at(&module, offset)` returns the type of a binding's declared name or the innermost expression, or `None` outside a target range.
Type queries inspect already-resolved types and do not execute Wasm.

<a id="束縛と式"></a>

## Bindings and Expressions

Identifiers consist of `$` and ASCII letters/digits, `_`, `.`, or `-`. `$__` is reserved for the compiler.
Bindings have lexical scope, and inner bindings may shadow outer names. Duplicate parameter or capture declarations are errors.

```text
(@let $name [(type T)] initial-value body)
(@let (($first [(type T)] initial-first) ($second [(type U)] initial-second) ...) body)
(@let {:field $binding ...} struct-value body)
(local.get $name)
(block [(result T)] unit-expression ... result-expression)
(if [(result T)] condition (then expression) (else expression))
(call $function argument ...)
(drop value)
(nop)
(unreachable)
```

Expressions are folded expressions or local-reference/expected-type numeric shorthands, and a function body is one expression. Use `block` to group multiple operations.
Every block expression except the last must have type unit; discard unused values explicitly with `drop`.
Conditions are `i32`, and branch types must match result. `unreachable` and `@recur` return no value and can appear in a position expecting any result type.
Evaluation proceeds left to right. Function definition order does not affect whether calls are allowed.
There is no local reassignment. Wasm `local.set` cannot be used in this language's source.

A multi-binding `@let` expands to nested `@let` expressions that evaluate initializers once each, from left to right. Each initializer can refer to preceding bindings and the outer scope, but not later bindings. A same-name binding shadows its predecessor only after its initializer has been evaluated in the previous scope. The body sees the last binding of a name. Type annotations can be omitted independently for each binding. This form groups initializations; it does not receive Wasm multiple results. Use the existing pattern let to decompose a pair.

`@let` accepts an optional type annotation, as in `(@let $x (type i64) 42 body)`.
With an annotation, the initializer is checked for conformance to that type, and the binding retains the annotated type.
For example, `$x` in `(@let $x (type (ref null $T)) (struct.new $T ...) body)` is nullable. The initializer expression itself remains non-null.

<a id="gc-データ"></a>

## GC Data

Structs and arrays use WAT syntax. Field names are optional, and access accepts a name or index.

```wat
(type $Node (struct (field $value i64) (field $next (ref null $Node))))
(type $Cell (struct (field $value (mut i64))))
(type $Ints (array (mut i64)))
```

Core instructions are `struct.new/get/set`, `array.new/new_fixed/get/set/len`, and `ref.null/is_null/as_non_null/eq`.
Named construction is `(@struct.new $T ($field initializer) ...)`. Every field of the type must be specified exactly once.
`(@struct.new $T {:field initializer ...})` and `(@$T::{:field initializer ...})` are sugar for it.
Directly within named initializers, bare numeric and text literals are also allowed according to the field's expected type. These two shorthands are independent and do not extend shorthand into general expression positions.
Initializers are evaluated once each in source order and finally arranged in type-definition order. Opaque, effect, ownership, identity, and mutability semantics are preserved.
For type arguments, use the existing `(@inst ...)` in the type operand of `@struct.new`. See [Named construction](docs/struct-construction.md).

`(@let {:field $binding ...} subject body)` extracts selected fields from a non-null shared struct. It evaluates the subject once, reads values in written order, and binds them to immutable locals visible only in the body. Even mutable fields retain their values at binding time. Unknown or duplicate fields, duplicate bindings, and empty patterns are rejected.
`(@struct.update $T object ($field $old new-value))` evaluates the object once, reads the old mutable-field value once for the update expression, and writes its normally returned value once. It returns unit and is limited to shareable, nonunit fields. The old-value binding is visible only in the update expression. Suspension and exceptions in that expression follow ordinary rules; if the final write is not reached, only effects already performed remain. The operation is not atomic. Both forms are sugar subject to existing type, effect, ownership, and opaque checks. [Details](docs/high-level-forms.md).
As in WAT, the element count of `array.new_fixed` is explicit. `set` is allowed only for declarations marked `mut`.
Null, array bounds, object identity, and allocation failure follow Wasm semantics.
Checking a nullable reference does not implicitly narrow its type. Use `ref.as_non_null` explicitly when needed.
There is no pointer/integer conversion or raw linear-memory access.

<a id="optionresult-の値型"></a>

## Option/Result Value Types

```wat
(func $make (result (@inst $option (@inst $result i64 i32))) (@effect pure)
  (@option.some (@inst $result i64 i32) (@result.ok i64 i32 (i64.const 42))))

(@match (@inst $option i64) (result i64) (@option.none i64)
  (@case none () (i64.const 0))
  (@case some ($value) (local.get $value)))
```

The types are `(@inst $option T)` and `(@inst $result T E)`. They can be nested and used for parameters, results, fields, array elements, explicit captures, and loop state.
The same combination of type arguments is instantiated as one internal type. Values with different type arguments are not converted implicitly.
To return a Result value from a function, write `(result (@inst $result T E))`. Context distinguishes this from standard WAT multiple-result declarations.

Construction uses `(@option.none T)`, `(@option.some T value)`, `(@result.ok T E value)`, and `(@result.err T E value)`.
Unit is also allowed as a type argument. That side has no payload, so omit the construction value and case binding.
Examples are `(@result.ok $unit i32)` and `(@option.some $unit)`. Unit itself cannot be used as a storage type for fields, arguments, or similar positions.

The match type operand takes a type constructor such as those above. Cover both none/some for Option and ok/err for Result, using explicit cases or a final default.
Payload arity, types, and exhaustiveness are checked. Even a case without a value requires the empty binding list `()`.
None and some(null), none and some(unit), and ok(unit) and err(unit) are distinct values.

Option/Result are semantic values, not types exposing their storage GC references. Their storage representation cannot be manipulated with ref.eq/is_null/as_non_null or struct.get/set.
If a payload contains an ordinary GC reference, its identity and mutability continue to follow the payload's own rules.
The backend generates private GC subtypes per type, distinguished as Type::Value in typed IR.

Shared private tuple/Option/Result storage may be eliminated when constructed locally and consumed only by matches/drops, including unused locals, multiple matches, branches, and bounded helper inlining. Payloads still evaluate once in source order, including defaults; ordinary payload identity and explicit traps remain. Fusion never crosses producer try/label/loop scopes. Escaping values, owned/resource/borrow functions, identity-bearing enums, unknown calls, and unproved cases keep their representation. Public ABI signatures and foreign tag/subtype checks remain. Private carrier allocation/OOM points can disappear. This compiler TCB pass adds no syntax or general multi-value calling convention. See [value optimization](docs/value-optimization.md).

<a id="閉じた-enum-と網羅的な-match"></a>

## Closed Enums and Exhaustive Match

Declare enums as `@variant` types. Case names are ASCII identifiers without `$`: the first character is a letter or `_`, followed by letters, digits, `_`, `.`, or `-`.

```wat
(type $Result (@variant (ok i64) (error i32)))
(type $List (@variant (nil) (cons i64 (ref $List))))

(@variant.new $Result ok (i64.const 42))

(@match $Result (result i64) (local.get $result)
  (@case ok ($value) (local.get $value))
  (@case error ($code) (i64.const -1)))
```

Each type has a closed set of at least one case. Duplicate case names are not allowed.
Each case has zero or more numeric or GC-reference payloads. Payloads are immutable and evaluated once each in declaration order during construction.
Recursive types and forward references are allowed; construction returns a non-null reference to that enum type.
Construction checks arity and types, so `ref.null` cannot be passed where `(ref $List)` is required.
If a referenced object is explicitly mutable, that object can still be changed.

`@match` has the form `(@match $Type [(result R)] subject (@case name ($bindings...) body) ...)`.
The subject must be a non-null reference to the specified enum type and is evaluated once.
Explicit cases can be ordered freely; each variant must be covered exactly once by a case or the final default. Unknown, duplicate, and missing cases are errors.
A final `(@default body)` may cover remaining variants without payload bindings. It must be unique, final, and nonredundant; omitted owned or borrowed payloads are rejected. Defaults retain type/effect/ownership checks and runtime tag/subtype traps. Write `@case`/`@default` explicitly; standard Component WAT `case` declarations retain their native spelling.

The binding count must match the payload; types come from the case declaration. Duplicate binding names within one case are errors.
Bindings are visible only in that case's body and may shadow outer bindings. Only the selected body is evaluated.
Each body's type must conform to result. Omitted result means unit. A match in tail position passes tail position to every case.
The final `@default` is the only catch-all. There are no guards or nested patterns; use a separate match for nested decomposition.

Source code cannot use `struct.new/get/set` on enum internals or treat one enum as another enum type.
`@variant.new` allocates every time, including for cases with no arguments. `ref.eq` compares reference identity; structural equality is not provided.

Wasm output contains a base GC struct with an immutable i32 tag and a final subtype for each case.
Each subtype stores the tag followed by only the payload needed for that case. Decomposition uses tag branching and checked `ref.cast`.
This base type is not sealed at Wasm boundaries. Unknown tags, the base type itself, or payload representations inconsistent with the tag supplied by external Wasm trap in defensive match checks.
There is no recursive prevalidation of external input. Checks run at each match, and effect analysis conservatively reports `may_trap=true`.
Enum construction also has `allocates=true` and `may_trap=true`, including allocation failure. Reading an immutable payload does not itself produce `reads`.

<a id="状態を引数で渡すループ"></a>

## Loops with State Passed as Arguments

```text
(@loop $label
  (bind $state T initial-value) ...
  [(result R)]
  body)

(@recur $label next-value ...)
```

Initial values are evaluated in the outer scope from left to right. State is readable inside the loop body.
A normally returned body value exits the loop. `@recur` is allowed only in the innermost loop's tail position.
Recur evaluates all arguments from left to right, then simultaneously updates all state and repeats.
Arity and types must match bind. This iteration becomes Wasm loop/br and does not grow the call stack.

<a id="for-マクロ"></a>

## For Macros

`(@for-each pattern list-value unit-body)` traverses an immutable list of shareable, nonunit elements in order. The list expression and length are evaluated once, and each element is read once. Patterns are a single name, tuple decomposition, or struct-field decomposition. An empty list does not execute the body; both body and overall form have type unit. Internal labels and indices are not exposed. Outer block/br or return can exit early. Expansion uses typed let/list/loop/if/block/recur, and existing checks reject repeated consumption of an outer owned value. [Details](docs/high-level-forms.md).

```text
(@for $label
  (bind $state T initial-value) ...
  (while i32-condition)
  (step next-state ...)
  unit-body)
```

Declare at least one state. Initial values are evaluated once each in the outer scope, left to right.
Each iteration evaluates the condition and exits on 0. Otherwise it evaluates the body, then step, and simultaneously updates all state.
Step must have the same arity and types as bind. If the body throws, that iteration's step is not executed.
Bindings are immutable within an iteration; no reassignment syntax is added. For expands into typed loop/if/block/recur IR.
The body and the entire for have type unit. There is no direct manipulation of for through recur inside its body or dedicated break/continue syntax.

<a id="wasm-例外の-trycatch-マクロ"></a>

## Try/Catch Macro for Wasm Exceptions

```wat
(tag $Error (param i32 i64))

(@try (result i64)
  (do (throw $Error (i32.const 1) (i64.const 42)))
  (catch $Error ($code $value) (local.get $value))
  (catch_all (i64.const -1)))
```

A tag is an ordinary Wasm tag. Payloads are zero or more numeric or GC-reference values, and throw arity/types are checked statically.
Throw has type never and returns no value. Its arguments are evaluated left to right and raised as an engine exception.
The try do expression and each handler must conform to result, which defaults to unit.
Catch bindings are immutable and visible only within that handler. Their arity/types must match the tag; duplicate names within a handler are rejected.
Duplicate catch tags are rejected, and catch_all can occur only last. Unmatched exceptions propagate outward; exceptions thrown in a handler are not caught by that same try.
Handler tail positions are retained, allowing tail recur to an outer loop.

Expansion uses current Wasm `try_table`, catch-target blocks, and local bindings for payloads. It does not substitute return codes or a custom exception heap.
**Traps are distinct from exceptions.** Even catch_all does not catch integer division by zero, null, array-bounds, unreachable, or other traps.
These rules follow [Wasm exception handling](https://github.com/WebAssembly/exception-handling/blob/main/proposals/exception-handling/Exceptions.md).
Standard WAT needing exception-reference capture or rethrow can use `catch_ref`/`throw_ref` directly. Exnref bindings and finally in typed macros are not implemented.
`may_throw` means the function may execute throw, including throws caught internally. It propagates from callees separately from may_trap.
Allocation and resource exhaustion from throw are conservatively included in effects. Handler reachability and complete exception elimination are not proved.

<a id="明示的なクロージャ"></a>

## Explicit Closures

```text
(@closure $C
  (@capture $captured T) ...
  (param $argument U) ...
  [(result R)]
  body)

(@closure.new $C capture-value ...)
(@closure.call $C closure-value argument ...)
```

Declarations occur directly under the module and each creates a distinct type `(ref $C)`.
A body can refer only to capture/param declarations and its own let/loop/for/match/catch bindings. Captures are not inferred.
Capture values are evaluated in declaration order at construction and stored as values. Capturing a GC reference shares its referenced object.
Environment fields are immutable. To change state, explicitly capture a reference containing `mut` fields or arrays.
Calls evaluate the closure value first, then arguments once each from left to right.

Each closure type has one body, so call targets are static. The environment becomes a GC struct and invocation an ordinary direct call.
This convention permits a closed call graph without manual capture-lifetime management.
There is no implicit conversion to another closure declaration with the same signature.

<a id="テスト宣言"></a>

## Test Declarations

```wat
(@test "name" i32-expression)
```

Declarations occur directly under the module; names are strings that are not solely whitespace. Duplicate test names within a module are rejected.
A body is checked as a function with no parameters or implicit captures and a fixed result type of `i32`.
It can use ordinary let/loop/match, explicit closure construction, and module functions/types.
As for other functions, a body of type `never` is accepted, but runtime traps and timeouts are test failures.
A normal return of 0 fails; nonzero succeeds. No implicit conversion from other numeric types is allowed.

Typed IR `Module.tests` stores the name, source range, body FunctionId, and execution export name.
Bodies use the same IR, effect analysis, and code generation as ordinary functions. Generated function names cannot be referenced from source.
Execution exports use names that do not collide with user exports; ordinary check/types/ir/analyze/build also process test bodies.
No mutable test-registration array or dynamic calling mechanism is generated inside Wasm.

File discovery, name filtering, result display, and time limits belong to the host `runner`.
Every test runs in a fresh instance. Sharing GC objects across tests or depending on execution order is not a language feature.
Filtering happens after type checking the whole module. Zero executed tests, or a module with type errors, is not treated as success.

<a id="最適化に使う意味論上の前提"></a>

## Semantic Assumptions for Optimization

Runtime object management is delegated to Wasm GC; no custom linear-memory heap or raw pointer arithmetic is introduced.
Locals, closure environments, and enum tags/payloads are immutable. Mutable struct/array changes are explicit in source and IR.
GC-reference aliasing is allowed, so different arguments are not assumed to refer to different objects.
Closure targets and all ordinary calls are static; adding tests does not change this property.

Optimization must preserve left-to-right evaluation order, executed branches, explicit mutation, reference identity, and defined traps.
Immutable-field reads have no mutable-memory read effect, but traps when reading nullable references are handled separately.
There is no facility for observing GC timing or object addresses, and no finalizers or weak references.
Imports conservatively have all effects; arbitrary user annotations cannot make them pure.
These facts are retained in typed IR and effect summaries. Typed-IR passes eliminate eligible private value containers and simplify validated immutable ranges. Range elimination uses a separate must analysis, not the diagnostic may summaries below; Wasm GC array bounds checks remain. See [value optimization](docs/value-optimization.md) and [range optimization](docs/range-optimization.md).

<a id="外部境界と解析"></a>

## External Boundaries and Analysis

Import/export are WAT function boundaries. The embedding host provides external function implementations.
Imports cannot carry annotations that underestimate effects; reads, writes, allocation, traps, divergence, and host effects are assumed conservatively.
All typed-core instructions are checked and converted to IR, from which analysis and WAT generation branch. The standard-WAT path uses the Wasm validator and assumes no purity for portions the core cannot analyze.
A false analysis result means that effect does not occur within this core model. True means possibility, not certainty.
Runtime stack exhaustion, host resource limits, debugger intervention, and similar events are outside this effect model.

<a id="effect-契約"></a>

## Effect Contracts

```wat
(func $add (param $x i64) (param $y i64) (result i64)
  (@effect pure)
  (i64.add (local.get $x) (local.get $y)))

(func $raise (@effect (throws $Error))
  (throw $Error (i64.const 42)))
```

Function and closure headers can specify `(@effect ...)`. When omitted, effects are inferred from the implementation.
Items are reads, writes, allocates, suspends, host, synchronizes, blocks, `(throws $Tag...)`, and `(throws *)`. Pure aliases the empty set and cannot be combined with other items.
The specified set is an upper bound on allowed effects. If implementation effects exceed it, the contract location and missing effects are reported.
Duplicate or unknown items/tags and multiple effect declarations are rejected. Contracts precede the body.

Reads/writes describe mutable GC data or linear-memory access; allocates describes creation of values whose identity is observable in source.
Ordinary struct/array, existing reference-type variant, and closure construction have allocates.
Storage allocations for Option/Result/pair/own and local delivery endpoints do not themselves have semantic allocates, but reads/writes/allocates and other effects from evaluating their payloads are always composed.

Throws is the set of tags that may escape the function. A corresponding catch removes body tags; catch_all also handles unknown body exceptions.
A handler's own throws are not removed by that same try. Result err adds no throws.
Callers inherit effects inferred from callee implementations, propagated to a fixed point including recursion. Declared contracts cannot hide actual effects.

Unknown imports have all effects and throws(*). User effect contracts on imports are rejected, separate from future trusted binding contracts.
Reads/writes/host in effect declarations are global upper bounds. Separately, analyze memory reports access targets and escape, while facts reports control-flow facts. Per-target contract syntax and callback effect type arguments are not implemented.

`analyze` displays semantic_effects and declared_effects. The existing effects conservatively describe physical GC allocation, internal throws, traps, divergence, and similar behavior.
Pure does not prove termination or absence of traps. Physical allocation failure and similar behavior remain in the existing effects.
When optimizing values whose GC representation is exposed to raw WAT/external code, semantic contracts alone cannot justify eliminating identity or allocation; lowering effects and escape must also be checked.

<a id="詳細な-memory--facts-解析"></a>

### Detailed Memory / Facts Analysis

Memory uses may-access paths rooted at parameter/capture indices, allocation-source function/position/internal slot, or unknown.
Members are fields, array elements, or variant/value payloads. As with existing semantic effects, reads/writes represent access to mutable GC slots.
Reads/writes, heaps storing references, returned references, references passed to the host, and references passed along exception paths propagate to a fixed point through the call graph and loop/recur.
Parameters and captures are substituted with actual-argument provenance; updates are weak updates retaining old values. Merely storing into a callee argument does not mean a caller's private allocation has escaped.
For each allocation, analysis computes reachability through return, storage in externally reachable slots, host calls, and exception paths. It follows cycles and container/capture references. Exception-path thrown references are conservatively included even if caught before leaking to the caller.
Parameters and captures can alias. An allocation is a site-based abstraction, not an object unique to each call/iteration. Reference paths deeper than 3 and unknown code widen to unknown.
After unknown writes or suspension, references in mutable slots widen to sets including unknown. Immutable fields and logical-value payloads themselves are not assumed mutable.

Facts carry function-local preorder node IDs and original spans. Macro-generated nodes can share spans, so a span alone is not a unique ID.
I32_range is an interval in 0..2^32-1; operations follow Wasm semantics, including wrapping and shift-count masking. Results that cannot be represented safely by an interval use the full range. I64 and floating-point interval inference are not implemented.
Analysis tracks unchanged local values/aliases and their relationship to immutable GC-array lengths. An unsigned `i < len` comparison, or a signed comparison known to be non-negative, is retained as bounds evidence.
When an ArrayNew length argument copies another array's length or a local value, that snapshot equality is retained too. It does not imply identity between the array objects.
Null checks and successfully completed dereferences refine non-null facts. Repeated reads of mutable fields are not assumed to produce the same value.
At branch joins, intervals are enclosed and only common conditions are retained. Loop-carried values widen to the declared type's upper bound instead of reusing initial values. Invariants and termination are not proved.
Normal_return_possible=false means normal continuation has been ruled out. True does not guarantee success. False bounds_proven/null_check_proven means unconfirmed.

These are diagnostic may analyses and local facts. They do not change existing effect-declaration/ownership acceptance or directly drive emitted Wasm. Non-escape, pure, or a diagnostic bounds flag alone never authorizes removing identity, evaluation order, or traps.
`proof/Flow.lean` checks signed/non-negative bounds conditions with BitVec. It does not prove soundness of the Rust abstract interpreter or all output; negative-index counterexamples, intervals/wrapping, aliases, loops, and calls are also covered by regression tests.

A separate bounded must-range pass tracks immutable snapshots, unsigned guards, and successful checked accesses on normal paths. Small shared-value helpers can be inlined with fresh locals/loop labels. Loop-carried values start unknown on every trip; mutable reloads and unsupported calls/control flow do not supply evidence. Proved subranges compose without offset wrapping. Redundant logical branches retain evaluation of their original condition; a proven fixed-width reader omits explicit helper guards while retaining Wasm GC bounds checks. Source cannot request unchecked reads. Unproved cases stay checked, and source validation precedes optimization. `proof/Ranges.lean` proves the arithmetic rules, not correctness of the Rust implementation or emitted Wasm.

<a id="wit-interface-の生成"></a>

## WIT Interface Generation

`walz wit file.walz` and `wit::emit(&module)` generate WIT from public typed-core functions.
This is a type contract using fixed package `walz:generated@0.1.0`, interface api, and world program. The `wit` command itself does not generate an executable. Executable Components are generated separately by the `component` command.
Internal and test functions are not exposed. Parameter names are p0/p1/..., and exports retain their original lowercase kebab-case names. WIT keywords are escaped; invalid export names are rejected.

Under the default boundary convention, i32/i64 map to s32/s64 and f32/f64 to same-named WIT types. There are no semantic unsigned-integer types yet.
`v128` has no direct WIT/JavaScript value-boundary mapping and is rejected, including when nested. Explicit conversion to scalars, bytes, or another supported representation is required. It can be used internally in Wasm.
Option/Result recursively map to the same WIT types. Result<Unit,E> becomes result<_,E>, Result<T,Unit> becomes result<T>, and Result<Unit,Unit> becomes result. Pair maps to tuple; own requires an explicit resource adapter.
Where WIT needs a unit-valued payload type, such as Option<Unit>, a helper enum with one case is used.
Future/Stream map to future<T>/stream<T>; unit payloads become future/stream without arguments.
Functions inferred to suspend receive async. Exports containing GC references unrepresentable in WIT or unhandled escaping exceptions require explicit adapters.
WIT does not retain arbitrary effect contracts, so a generated interface alone cannot justify assuming another function is pure.

<a id="所有値の最小規約"></a>

## Minimal Rules for Owned Values

```wat
(@let $ticket (@box.new i64 (i64.const 42))
  (@match (@inst $box i64) (result i64) (@move $ticket)
    (@case own ($value) (local.get $value))))

(@let $unused (@box.new $unit)
  (@box.close (@move $unused)))
```

`(@inst $box T)` is a nonduplicable value wrapping shareable data T. T can be unit, in which case omit the own.new value argument and case binding.
Own.new creates a new owned value, and move transfers ownership from a local, parameter, or capture. Local.get cannot be used on owned values. Reuse after move, moving an unowned value, and dropping an owned value are rejected.
Consume own in a case own match to extract its contents, or explicitly with own.close. There is no implicit release.
Current own.close consumes the owned-data wrapper and expands to Wasm drop. It does not implement OS-resource close or Future/Stream cancel.
Own does not make its payload GC reference exclusive. Existing shared GC-reference aliases and mutability remain unchanged. Directly wrapping another owned value inside own is not allowed.

`(@inst $tuple T U)` is constructed with `(@tuple.new T U first second)` and decomposed by case pair match. Omit value arguments and bindings for unit components.
Pair, Option, and Result are owned if any component is owned. A closure capturing owned values is also owned; use move for owned captures at construction and for its environment at invocation.
An owned closure cannot be nullable or have its reference observed with ref.eq/is_null/as_non_null. Invocation consumes it, and its body must also handle its owned captures.
Storing owned values in shared struct/array/reference-variant fields or exception payloads is rejected.

In a normally returning scope, owned parameters, captures, let/case bindings, and loop state must be handled by move, return, decomposition, or close.
The initial version does not permit free affine abandonment; within this scope it uses linear completion rules. Shared values remain reusable any number of times.
Normally joining if/match branches must agree on the set of remaining owned locals. After evaluating simultaneous-update arguments, recur must match the outer owned set to the loop entry and leave no old owned state behind.
State can be moved into the next state, or old state consumed and new state created.
Resource release at the end of a nonterminating loop or on trap/OOM is not proved. Before an explicit unreachable, owned values present at that point must be handled.

Owned values can coexist with try/throw and throwing calls. Checks include ownership sets at catch/branch/iteration entry and owned operands partway through evaluation. Exception paths that lose owned values are rejected.
Unknown imports passing general owned values, owned exception payloads, partial moves, and general lifetimes are not supported. External resources have synchronous destructors and nonescaping borrows. See [Control and ownership](docs/core-expressions.md).

<a id="配送済みの-future-と有限-stream"></a>

## Ready Futures and Finite Streams

```wat
(@let $future (@future.ready i64 (i64.const 42))
  (@future.await i64 (@move $future)))
```

Future/Stream are opaque owned values corresponding to readable ends. Direct match/ref operations, storage in shared GC fields, and implicit drop are not allowed.
Ordinary core Wasm output handles ready data stored in Wasm GC. Component output also connects pending data and the JSPI event loop. Shared-payload writers and scoped spawn/join are supported too. See [Writers and child tasks](docs/scoped-tasks.md).
Payload T is limited to shareable data. The initial version rejects payloads containing owned values or other Futures/Streams to prevent silent abandonment at completion.
Combinations of shared data such as Result, GC references, and unit are allowed. Even for GC-reference payloads, sharing and mutability of the referenced object remain unchanged.

| Operation | Type and meaning |
|---|---|
| `(@future.ready T value)` | Creates Future<T>; omit value for unit |
| `(@future.await T future)` | Consumes the endpoint and returns T; has suspends |
| `(@future.close T future)` | Abandons receipt and consumes the endpoint; returns unit |
| `(@stream.empty T)` | Creates a terminal Stream<T> |
| `(@stream.cons T value rest)` | Creates Stream<T> with value at the head and consumes rest; omit value for unit |
| `(@stream.next T stream)` | Consumes the endpoint and returns pair<Stream<T>,Option<T>>; has suspends |
| `(@stream.close T stream)` | Abandons remaining receipt and consumes the endpoint; returns unit |

Construction arguments are evaluated once each, left to right; the read subject expression is also evaluated once.
An awaited Future cannot be reused. After next, use only the returned Stream.
When an element exists, next returns a new endpoint and some(value); at the end it returns a terminal endpoint and none. Reading the end again returns none. An owned value is returned even at the end, so a final close is required.
None is distinct from some(unit)/some(null). Stream termination alone does not determine the success or failure of a separate completion Future.

Await/next always have suspends as suspension boundaries. Ordinary core Wasm returns ready data immediately; Component output also waits for external delivery.
Construction and close themselves add no host/suspends. Physical GC allocation and checked-cast traps remain in existing low-level analysis.
Closing a local endpoint only consumes it. External Component endpoints lower to drop-readable, prompting host dispose/iterator.return. Cancellation of OS I/O, ongoing reads/writes, or the entire producer task is not guaranteed.

`examples/ready-delivery.walz` creates pair<stream<i64>,future<result<unit,i32>>>, reads 40 and 2, closes the terminal endpoint, checks completion, and returns 42.
The same ownership contract can be checked through run-verified and Component generation. Component boundaries use the Canonical ABI adapter in the next section.
Passing owned endpoints through raw Wasm exports remains rejected in verified execution.

`proof/Delivery.lean` proves in the model one-shot Future delivery, rejection of closed endpoints, retention of Stream ownership even at termination, and agreement of repeated next with take/drop.
This model is also included in generated Lean source. Preservation of actual Wasm semantics is outside the proof scope; delivery order, termination, unit, GC references, and evaluation counts are compared in execution tests.

## Bench

```wat
(@bench "integer mix" (param $i i32) (result i32)
  (^ (* $i 16777619) (>>> $i 7)))
```

`@bench` is an execution declaration directly under a module. In order, it contains a nonempty name string unique within the module,
an optional `(param $i i32)`, an optional `(result type)`, and one expression.
Omission means no arguments and unit. Its body receives the same type, effect, and ownership checks as an ordinary function.
The result must be shareable or unit; owned values and borrows cannot be returned. Its name cannot be referenced as a source function.
IR `benches` stores the name, Span, FunctionId, and export name. No new expression semantics are added.

`walz bench` recursively discovers `*_bench.walz` below the current directory by default.
Execution is limited to synchronous, closed guest code. The runtime harness does not accept host imports or suspension.
Each case runs in a separate process, with a Wasm batch loop calling the body with iteration numbers 0 through N-1.
Results are stored in a public global, retaining only the last GC result. Measurement loops/globals belong exclusively to the runner
and are not added by ordinary compile. See [Benchmarking](docs/benchmarking.md) for measurement conditions and CLI details.

<a id="law-と契約付き実行"></a>

## Laws and Verified Execution

```wat
(func $add (param $x i32) (param $y i32) (result i32) (@effect pure)
  (i32.add (local.get $x) (local.get $y)))

(@law "addition is commutative" (param $x i32) (param $y i32)
  (i32.eq (call $add (local.get $x) (local.get $y))
          (call $add (local.get $y) (local.get $x))))
```

A law has a name, zero or more parameters, and an i32 condition expression. Duplicate names are rejected; the condition must be nonzero for every parameter input.
Conditions use existing expressions rather than a separate predicate language. A law body's effect upper bound is pure. Laws become dedicated source-generated functions with no exports.
The actual law function and its callees' typed IR are translated to Lean BitVec, and bv_decide proofs are checked by the Lean kernel. All i32/i64 bit patterns are covered, preserving integer wrapping, signed comparison, and Wasm shift-count masking.
The initial version supports only i32/i64 constants and supported nontrapping operations, local/let/block/if, nonrecursive direct calls, and unit. Division/remainder, recursion/loops, floating point, GC/owned values/exceptions, and similar features are rejected as outside the law semantic model.
Execution/output is rejected if proof fails or the verifier is unavailable. Lean maxHeartbeats is 4,000,000; properties not proved within the budget are also treated as unproved.

`verify` and `run-verified` accept only the typed core and reject all imports, raw Wasm exports passing owned values, and public functions without effect upper bounds. Generated test/bench functions need no effect declarations.
Ownership-local contracts are extracted to JSON and checked by an independent Lean checker; Wasm is then generated from that same IR and validated. Users cannot weaken contracts to hide implementation effects or ownership operations.
For modules containing laws, ordinary run/build/wat/expand and CLI check also run these checks. The ordinary WAT path without laws remains unchanged.
Rust API check and CLI types/ir/analyze serve checking/investigation and do not prove laws. Proof cannot be skipped by compile producing an executable or by verified::compile with contracts.

`contracts` exposes ownership obligations, and `proof` exposes generated Lean source. The verifier is pinned to Lean 4.34.1.
`proof/Ownership.lean` proves that accepted contracts satisfy declarative ownership-set rules (check_sound) and reject double consumption (double_take_rejected).
These are proofs of the contract model. Parsing, Rust typing/effect analysis/contract extraction/BitVec translation, Wasm lowering, and runtime/adapters remain trusted implementations. They do not prove whole-compiler semantic preservation, termination, or I/O success.
As in the referenced [Bend law/proof and quantity](https://github.com/bendlang/bend/blob/3ca3df3209ebb0fac4b80e41dac18b41442e6c58/guide/GUIDE.md), contracts, implementation, and verifier are separated. Bend's dependent type theory itself is not introduced.

<a id="設計規約-effect-と-wasi-030-の対応"></a>

## Design Rules: Mapping Effects to WASI 0.3.0

This section records implementation status and design rules for further extensions. Value types, effect contracts, ownership, ready-only Future/Stream, WIT type generation, laws and Lean checking, analysis `may_suspend`, and the [WIT shape contract](interfaces/async-contract.wit) are implemented.
Pending reads on external readable handles, Component adapters, timer bindings on JSPI, and cooperative task cancellation are implemented. Cancellation terminates an ongoing read. Shared-payload writers and language-side producer/consumer tasks are also implemented.

References are the [provided Verse effect model](https://verselang.github.io/book/13_effects/) and [WASI 0.3.0](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0).
Effects are included in function types and checked as upper bounds on permitted behavior. The initial rules do not include rollback-based failure, prediction, or Verse-specific restrictions on effect combinations.
Returning err from a computation performing external I/O does not roll back I/O already completed.

<a id="データ型と制御を分ける"></a>

### Separate Data Types from Control

| Language semantics | WIT mapping |
|---|---|
| `Option<T>`: none / some(T) | `option<T>` |
| `Result<T,E>`: ok(T) / err(E) | `result<T,E>` |
| `Result<Unit,E>` | `result<_,E>` |
| `Future<T>`: owned handle receiving a single delivery | `future<T>` |
| `Stream<T>`: owned handle receiving an ordered sequence | `stream<T>` |
| Function returning a handle before waiting for later delivery | `func(...) -> future<T>` |
| Function waiting for completion of the call itself | Use `async func(...) -> T` as the basic binding policy |

Option/Result are semantic value types, separate from GC storage layout. None is not equated with some(null).
The identity of storage GC objects themselves is not observable through value types; semantic payload identity follows that payload type's rules.
WIT results with omitted success/error payloads and argumentless future/stream delivering unit are also normalized at boundaries.
T/E must be representable in WIT at the boundary; WIT type checking is reused.
Internal cyclic GC graphs, closures, and GC references are not directly exposed as WIT values. Explicitly convert to a value representation or resource handle when needed.

Result err and Option none are ordinary values. Merely returning them does not produce throws/decides effects.
`future<Result<T,E>>` is an ordinary allowed combination; suspends may also be combined with Result.
Result-propagating `@result.let` / `@result.do` expand to match and return that expression's Result. They do not become Wasm throw or implicit function return.
Wasm exceptions, traps, task cancellation, interrupted reads/writes, delivery completion, stream termination, and application err are handled separately.
Where internal exceptions need conversion to a public Result, explicitly identify the tags and error variants in an adapter.

<a id="関数の-effect-と推論"></a>

### Function Effects and Inference

A function type is conceptually `Fn(Params) -> Results ! Effects`; implementation-inferred effects must fit within the declared upper bound.
Inference is the default for internal functions; public APIs and bindings retain contracts. The Wasm validator checks instruction types, and effect checking checks additional behavioral contracts.
Callback types also retain effects; generic functions compose callback effects into their own.

| Family | Tracked facts |
|---|---|
| Heap | Per-region reads/writes and creation of observable new identities |
| Protocol state | Per-channel/endpoint reads/writes; delivery, close, and cancel state changes |
| Suspension | Logical suspension/resumption allowing other work to intervene |
| Exceptional control | Set of throws tags escaping the function |
| Divergence | Possibility of nontermination |
| Host | Capabilities used, such as filesystem, network, and clock |

Current `analysis.allocates` describes possible physical GC allocation. Allocation for semantic values is not conflated with the effect of creating values distinguishable by reference identity.
Semantic function contracts and post-lowering analysis such as GC allocation/traps are retained separately.
Current `may_throw` conservatively includes internally caught throws. Public throws contracts and semantic_effects use tags that still escape after handling.

`may_suspend` does not mean GC pauses or OS scheduling. Await/next and unknown imports/opaque WAT conservatively set it to true, and it propagates to callers.
WIT describes types and async calling forms but does not prove reads/writes/purity of arbitrary functions.
Narrowing import effects requires a trusted binding contract corresponding to the implementation. Users cannot bypass checking by declaring arbitrary imports pure.

Suspension points are optimization boundaries for shared mutable state. Combining reads before and after await requires proof that the target is immutable or cannot be modified by another task.
For computations returning a future or starting a task, producer effects remain in task/scope summaries. Removing suspends from the caller does not imply that started I/O or mutation has disappeared.
A Future is a delivery endpoint, not the producer's function type or the task itself.

<a id="所有権と終了処理"></a>

### Ownership and Cleanup

GC objects can continue to share references. Local Future/Stream values are owned and cannot be reused after move. These rules also apply to future external endpoints and resource own handles.
Ownership propagation through Option/Result/pair/closure containing owned values is implemented. These rules will also apply to future resource types/records. Owned values cannot be stored in freely shared user GC fields.
Owned slots in compiler-generated async frames are managed separately as internal representations with checked state transitions and release.
WIT future/stream values correspond to readable ends. Writers use separate internal types and are not implicitly passed as ABI future/stream values.
Duplication or repeated await requires a separate explicit abstraction for sharing/buffering.

Endpoints and ABI buffers with a read/write in progress are borrowed and cannot be reused, moved, or freed before completion notification or confirmed interruption.
Closures capturing owned handles also use moves and are checked as once-call if the body consumes the value.
Capture declarations are retained, with syntax requiring explicit moves of owned captures.
Borrowed resources are valid only for a call's duration, including throughout an asynchronous call.

Closing an endpoint, canceling an ongoing read/write, and canceling a task are separate operations. Dropping a handle alone does not mean the producer task has been forcibly terminated.
Task cancellation separates request from acknowledgment, including races with normal return according to the runtime protocol.
Cleanup does not depend on GC finalizer timing. Ownership release is defined on normal completion, explicit transfer, exception, and cancellation paths.
The model represents producers continuing after a function returns stream/future, distinguishing return-value delivery from producer-task completion.

<a id="wasi-030-の-io-形状と-abi"></a>

### WASI 0.3.0 I/O Shapes and ABI

Streaming I/O uses `tuple<stream<T>, future<result<_,E>>>` as its basic shape.
Stream termination alone does not establish success of the entire operation; the completion Future's Result must also be checked. Errors are not automatically pushed into each element's Result.
Backpressure, chunked reads/writes, and partial progress remain part of the adapter protocol.

Internal values and continuation state live in Wasm GC; Component boundaries lift/lower according to the [Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md).
Endpoint ownership, suspension, and cleanup are based on the [Component Model concurrency design](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md#streams-and-futures).
The boundary layer using linear-memory buffers and handle tables is separated; internal GC references are not assumed to pass directly through the ABI.
WASI packages are pinned to `@0.3.0`, and Component Model/generation-tool/runtime combinations are recorded. Future gated features on main are not automatically added to the 0.3.0 contract.

`interfaces/async-contract.wit` is a custom test interface for validating these mappings, not an implementation of wasi:* interfaces.
`just wit-check` checks the WIT encode/validate/decode round trip. This checks type shapes and does not establish asynchronous execution on a WASI runtime or completion of the GC adapter.
Shared-payload writers, scoped spawn/join, WASI 0.3.0 file-value adapters, and user-defined monomorphization are implemented. Child-only cancellation/deadlines and native own/borrow for imported resources are supported. Full native WASI resource worlds, select, and owned-payload delivery remain future work.

<a id="実装した-component-と-jspi-の契約"></a>

### Implemented Component and JSPI Contracts

`component source.walz output.wasm` accepts only the typed core. It checks public-function effect bounds, fixed import names/signatures, ownership, and laws, then generates a Component from the same IR checked through Lean.
The public API is `walz:generated/api@0.1.0`. Numeric values, Option/Result, pair, and Future/Stream are converted recursively. Public GC references and own boxes, unknown imports, and unhandled exceptions are rejected. Internally, cyclic GC graphs and closures are allowed.

Internal shared values use GC value representations; external Future/Stream values are private GC subtypes containing readable handles. Await/next evaluate their subject once and consume the endpoint.
External await lowers to `canon future.read`; next lowers to `canon stream.read`. JSPI suspends the stack until the synchronous Canonical ABI read completes. Wasm GC references remain reachable on that stack.
Stream read count and status are interpreted separately, preserving a final element delivered together with DROPPED. At the end, the external handle is dropped and replaced with a rereadable local terminal endpoint.

Boundary linear memory is managed with an upper limit of 16 MiB and 8-byte alignment. Layout of numbers, variant discriminants/payloads, and tuples follows the Canonical ABI; arguments exceeding 16 flat parameters are indirect.
Indirect arguments are converted to GC values before their original region is freed. Compound results return call-specific regions, freed by post-return. In-progress read regions are freed after read completion and lifting.
After a trap, the entire instance is assumed discarded. Buffer reuse after disposal and automatic cancellation of all external operations on trap are not guaranteed.

Ready Futures and finite Stream prefixes are passed to generated `walz:runtime/delivery@0.1.0` factories, returning Promises or pull-based AsyncIterables.
Because producers do not run inside the guest, there is no cycle where the guest call cannot return until its return value is consumed. An external Stream tail can be connected to prefix elements, and close also propagates to an unread tail.
Unit becomes no payload in future/stream, an omitted payload in Result, and a one-case `walz-unit` enum in Option/pair elements. None and some(unit), and a unit element and EOF, remain distinct.
`.wasm.json` is a version=2 manifest recording factories and async exports and is distributed with the Component.

The only trusted event-loop imports are sleep/yield/defer-i64/ticks/fallible-i64 from `walz:host/event-loop@0.1.0`. Types as well as names must match. All have host; sleep/yield also have suspends.
Defer, which only returns a Future, has no suspends but retains the host effect of starting a timer. No facility is added for users to narrow arbitrary import effects.
Endpoint-read/close ownership contracts are retained in generated output. Current global effect sets do not include per-endpoint protocol state; pure alone never justifies eliminating ownership operations.
Low-level allocation/trap information in `analysis` summarizes existing core lowering, not all generated ABI glue and host operations.

JS execution is validated with Wasm GC and JSPI in Node 24.21.0 / Chromium 156, plus jco-transpile 0.23.0. Native Component async functions are lifted/lowered through the synchronous Canonical ABI; calls to one instance are serialized. The JavaScript event loop continues during waits.
For generated JS from this pinned version, `runtime/jco-compat.mjs` corrects unit buffer metadata, undefined Stream elements, and completion processing of failed async imports. Corrected glue is also trusted implementation.
Lean's Delivery model remains ready-only. It does not prove pending states, ABI semantic preservation, jco scheduling, or Promise completion. These integrations are compared through Node/Playwright execution.

<a id="corejspi-の-js-専用-abi"></a>

### JavaScript-Specific Core+JSPI ABI

`core-js source.walz output-directory` performs the same type, effect, ownership, and law checks as Component output above, generating `core.wasm`, `memory.wasm`, `bindings.mjs`, and `manifest.json` from the same IR.
`runtime/build-core.mjs` adds shared `core.mjs` / `host.mjs` / `tasks.mjs` and `index.js`. JS build/execution does not load jco.
Public types, trusted imports, unhandled exceptions, and delivery-payload restrictions are shared with Component output. Public GC references/own boxes and delivery of owned payloads/nested Futures remain rejected.

Manifest `abi` is `walz-core-js-v2`. The Core loader checks this value and also accepts v1 numeric bindings. Generated Wasm, bindings, and the shared host are distributed as one matching ABI set.
This is a private JS ABI, not an implementation of the standard Component Canonical async ABI or WASI future/stream handles.
Memory layouts and the arena for numeric values, variants, pairs, and bytes/text share the existing adapter. Arguments are always indirect: if any exist, one pointer to a packed tuple is passed, and the original region is freed after all are lifted to GC values.
Nonunit results also return a pointer; after generated JS copies values/transfers endpoints, post-return in finally frees the region. Unit results pass no value.

In JS, i64 uses bigint and other numbers use number. S32/s64 ranges are checked on input. Bytes are copied as Uint8Array and text as string.
Pairs are two-element arrays and Result is `{tag:'ok'|'err', val}`. Unit is `'value'` within Option/pair and undefined for omitted payloads of Result/Future/Stream.
Option represents none as undefined and some as its payload; when the direct payload is itself Option, the outer layer uses `{tag:'none'}` / `{tag:'some',val}`.
Per-type read/write functions are generated for public APIs and portable Future/Stream payloads within the module. No JS codec is generated solely for private GC types or internal text.

Only external Future/Stream values receive positive u32 IDs in a per-instance JS table. There is no GC-object root table.
Await/next suspend a JSPI import and write to a temporary region after delivery. Registration is removed on Future consume/close, Stream EOF/close, and cancellation.
Endpoints returned across the boundary are removed from the table and ownership passes to the caller; their JS wrappers do not retain the Wasm instance. IDs are not reused, and exhaustion of u32 fails.
Completion of a synchronous task returning a Future is distinct from delivery of that Future itself. An unread returned Future can be explicitly closed or passed back to a guest function that closes it.

Calls to the same instance are serialized through existing Tasks. They share `instantiate()` options `api` / `start`, `getCoreModule`, `instantiateCore`, `eventLoop`, `imports`, and `onTaskEvent`.
If JS input conversion fails before guest entry, only endpoints and buffers registered by that conversion are released. A Wasm trap or host failure closes the instance and rejects subsequent calls.
Host failure is fatal even if caught by source catch_all, and fails the outer call. The original reason is retained in the Error itself or RuntimeError.cause; rejection with 0 or undefined is also failure.
Cancellation detaches unfinished reads, unwinds through Wasm checkpoints and a dedicated exception, and frees buffers. No late-producer callback remains that could write into freed regions.
Arbitrary Promises are not forcibly stopped. Cancellation completion is reported after host dispose / iterator.return finishes. Detailed state transitions are shared with the next section.
This integration's semantic preservation is not added to the Lean proof scope. The same Node/Chromium tests run on both outputs, measuring endpoint registrations, arenas, and GC reachability.

<a id="jspi-タスクの協調的取消し"></a>

### Cooperative Cancellation of JSPI Tasks

`start(jsExportName, args, {signal?})` returned by `instantiate()` enqueues a task on the same instance. A task has readonly state and result plus `cancel(reason?)`.
State is queued/running/cancelling/returned/cancelled/failed. Cancel returns an acknowledgment Promise. Its value is `{status: returned|cancelled|failed, started: boolean}`; repeated requests return the same Promise and retain the first reason.
Result settles as the guest's normal result, TaskCancelledError, or execution/cleanup failure. Cancellation reason is TaskCancelledError.cause. It is not implicitly converted to public API Result or a throws tag.

Canceling queued work never enters the guest or passes arguments. Canceling running work sets a request flag and generates cancellation-completion events for in-progress native Future/Stream reads. Waiting sleep/yield requests host dispose.
Guest buffers are not freed until Canonical read completion or cancellation notification. Checkpoints after reads and host calls check the request and throw a dedicated internal tag before lifting uninitialized payloads.
Source try/catch lowering detects this tag before user handlers and rethrows it outward. Even catch_all cannot absorb cancellation. The outer ABI wrapper catches it and releases owned endpoints/buffers before returning to JS.

External readable handles are registered in a private GC ownership list on creation. Consumption, close, or boundary transfer removes them; cancellation drop-readable applies only to remaining endpoints. The list also tracks endpoints moved into owned closures or tuples.
The allocator maintains a doubly linked list of live blocks, removing them on ordinary free. Remaining ABI regions are free-all'd only after cancellation unwind starts.
Cancelled is reported only after all async host cleanup started within the scope completes. Cleanup failure returns failed and AggregateError and rejects subsequent calls. Successful cancellation allows instance reuse.

An already-settled normal return wins as returned. Returned Future/Stream values belong to the caller and are not dropped by later task.cancel. Waiting for Future delivery is not confused with completion of a nonasync function returning it.
Direct synchronous APIs return synchronous values and reject as busy while another task runs or cleans up. Start can also enqueue synchronous functions. Direct async API calls remain serialized through the queue.

Cancellation does not forcibly stop CPU work or roll back effects already performed. If host operations or dispose never finish, acknowledgment may also never finish.
The current [Component Model cancellation rules](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md#cancellation) deliver requests and acknowledgment to callback-ABI tasks. This implementation is Walz's synchronous-ABI/JSPI control layer and does not directly use native `subtask.cancel` / `task.cancel`. Writers and language-side producer tasks connect to existing ownership and JSPI. No syntax is added here for guest-side child-only cancellation.
Jco corrections include pending-copy cancellation wiring, avoiding continued waits on host iterator.next after cancellation completion, and host-cleanup acknowledgment. These are also part of the runtime TCB.

`proof/Tasks.lean` checks in a state model that a request does not complete cleanup, buffers cannot be freed before reads end, acknowledgment is rejected before all resources/host cleanup finish, and cancellation after return is ineffective.
It does not prove model/implementation correspondence, Wasm unwind, jco copy/table implementation, or external-host termination. Node/Chromium tests compare cancellation before/after start, during read, and after return; late delivery; delayed/failed cleanup; reuse; and memory reuse.

<a id="bytes--text-と-abi"></a>

## Bytes / Text and ABI

`$bytes` is an immutable arbitrary byte sequence; `$string` is an immutable validated UTF-8 value type.
Both are non-null and shareable, without exposed identity, storage references, or writes.
Internally, a wrapper holds a private packed-i8 GC array; mutable-array writes are restricted to construction.
Storing numbers into bytes uses the low 8 bits, as with Wasm i8.

| Expression | Result and rules |
|---|---|
| `(@bytes.new i32-expr...)` | `$bytes`; empty is allowed; evaluates each expression once from the left |
| `(@bytes.from-array $Array array-expr [start length])` | Copies all or a range of a non-null i32/i8/i16 array into immutable bytes; selected values outside 0..255 trap; packed values are read unsigned |
| `(@bytes.copy-to-array $Array destination at bytes start length)` | Copies immutable bytes into a mutable packed i8 array; returns unit and traps on invalid ranges |
| `(@bytes.len bytes-expr)` | Byte count as `i32` |
| `(@bytes.get bytes-expr index)` | `i32` in 0..255; unsigned index; out of bounds traps |
| `(@bytes.slice bytes-expr start length)` | `$bytes`; requires start <= size and length <= size-start; out of bounds traps |
| `(@string.lit "日本語")` | `$string`; source literals are UTF-8 |
| `(@string.byte-len text-expr)` | UTF-8 byte count as `i32` |
| `(@string.bytes text-expr)` | `$bytes`; preserves content; private storage can be shared |
| `(@string.from-bytes bytes-expr [start length])` | `(@inst $option $string)`; none for invalid UTF-8, some otherwise; explicit ranges validate before copying |
| `(@bytes.is-utf8 bytes-expr [start length])` | i32 0/1; strict validation of the complete value or selected range, without a payload allocation |

Invalid continuations, truncated sequences, overlong encodings, surrogates, and values above U+10FFFF are rejected.
NUL and BOM are retained as ordinary data. Normalization, graphemes, and code-point indexing are outside this API; search/comparison/concatenation are provided by the core library.
Text byte-len differs from JS string.length. An arbitrary byte subsequence need not be UTF-8, so conversion to text always validates it.

Optional ranges use unsigned i32 start/length and require start <= size, then length <= size-start. Operands evaluate once from left to right before bounds/UTF-8 checks. Out-of-bounds ranges trap; bounded invalid UTF-8 returns 0/none. Empty ranges are valid regardless of surrounding bytes. Validation and ASCII lookahead read only the selected range. A ranged `@string.from-bytes` validates before allocating/copying exactly that range into independent immutable storage; the one-argument form retains its existing sharing behavior. `$bytes-view.is-utf8` and `$bytes-view.to-string` expose these operations through opaque views. `$string-view.from-bytes-view` returns option<string-view> sharing the validated range, allocating only small GC headers. Views retain the original buffer, and later string-view slices require UTF-8 boundaries. Materialization detaches storage and currently revalidates. Type/effect/ownership/opaque checks remain unchanged; the validator/lowering remain compiler TCB. See [views](docs/views.md#validate-a-byte-range-as-a-string-view).

WIT maps bytes to `list<u8>` and text to `string`. Internal GC references do not cross the ABI.
Using the UTF-8 setting of the [Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md), both flatten to two i32 values, ptr/byte-length, and use size=8 / align=4 in linear memory.
Inputs are copied into GC and input buffers freed; outputs are copied to dedicated buffers and freed by post-return.
Empty inputs also accept nonallocated pointers without freeing them. Option/Result/pair payloads are processed recursively.
Future/Stream payloads are received through read realloc; temporary regions passed to delivery factories are freed, including compound payloads.
Total adapter linear memory is limited to 16 MiB. Out-of-space or invalid ABI input traps, distinct from text.from-bytes none.
In JS, bytes are Uint8Array and text is string. UTF-8 encoding of lone surrogates during string lowering follows the JS binding's TextEncoder.

These operations themselves have no semantic reads/writes/allocates. Argument effects are composed.
Physical copying, validation, allocation, and possible failures remain in low-level analysis.
Hosts accessing raw core Wasm directly are outside the TCB protecting this private representation and must preserve value-type immutability.

<a id="静的モジュール"></a>

## Static Modules

One file is one module; declarations are private by default. There is no `@namespace`.

`math.walz`:

```wat
(module
  (@export (func $add) (type $Box))
  (type $Box (struct (field $n i32)))
  (func $add (param $n i32) (result (ref $Box))
    (struct.new $Box (i32.add (local.get $n) (i32.const 2)))))
```

`main.walz`:

```wat
(module
  (@use "./math.walz" (func $add $make) (type $Box))
  (func $main (export "main") (result i32)
    (struct.get $Box $n (call $make (i32.const 40)))))
```

`(@export (func $f) (type $T) (tag $Error))` publishes declarations for static linking. `@public` is synonymous. A closure's func and type can be published separately.
`(@use "./file.walz" (func/type/tag $member [$alias])...)` binds only selected public declarations. Omitting an alias uses the original identifier.
Built-in types/functions are selected from `(@use "walz:core" ...)`. File paths begin with `./` or `../`. Absolute paths, namespace specifications, and cycles are rejected.
Definitions at the same canonical path share type identity; private names in other files and imports in dependencies are not implicitly imported.
A dot is an identifier character, not a separator. To use `$math.add`, declare that identifier or bind it as an alias through `@use`.
Imported aliases can be re-exported with `@export`. Collisions, undefined names, and access to private names are rejected. There are no wildcards, default exports, dynamic loading, or initialization code.
`@export (type $T)` exposes representation; `@export (opaque $T)` restricts operations to the declaring file.

`@include "relative.walz"` textually inserts declarations without creating scope. Paths resolve relative to the file containing them.
`source::load` / the CLI resolves files; string-based check/compile APIs perform no file I/O. An unresolved file `@use` yields `E_SOURCE_REQUIRED`.
Limits are depth 64 and 16 MiB after expansion; diagnostics map back to original file positions.

After type/effect/ownership/opaque checks on all definitions, definitions unreachable from ABI exports and similar roots are eliminated, and remaining definitions statically link into one Wasm. There are no runtime module objects.
Native WAT `(import "host" "member" ...)` and `(export "name" ...)` retain their meanings, separately from `@use` / `@export`.
See [Static modules and DCE](docs/static-modules.md).

`(@use "./math.wasm" (component $Math))` selects a binary constructor. Use explicit `instantiate`, `alias`, and `@connect` for state and imports; function/type selection from `.wasm` is rejected. Async and owned ports use native Component contracts. Unused explicit instances retain initialization. [Examples and limits](docs/wasm-use.md).

<a id="値だけを渡す-text-io-capability"></a>

## Value-Only Text I/O Capability

`walz:host/text-io@0.1.0` is a fixed trust boundary. `read-lines` returns `stream<result<text,i32>>`; `write-line(text)` returns `future<result<unit,i32>>`.
Ordinary I/O failures use Result; host contract violations and cleanup failures are execution failures. Both functions have host effect; endpoint-returning calls themselves do not suspend, while await/next have suspends.
Names/types are checked by `bindings::validate`; public-function effect bounds, ownership, and laws use existing checks. No language syntax or ownership rules are added.

Component and Core manifests record module/name/jsName/params/result/capability/effects for each import.
Required capabilities are text-input for read-lines and text-output for write-line. JS `options.imports` is keyed by versioned module names and explicitly supplies declared functions.
JS implementations and I/O operations are TCB; the manifest itself does not prove JS effects. Default `eventLoop` bindings add no filesystem/network authority.
Component creates a separate import instance per namespace; Core-generated bindings copy typed arguments to the host.
Core artifacts are regenerated as ABI v2. New loaders also accept v1 output; old loaders reject v2.

<a id="行の表現と終了"></a>

### Line Representation and Termination

`createTextIO` takes a byte iterable as input and `write(Uint8Array, AbortSignal) -> Promise<void>` as output. Either can be provided alone; guests requiring an absent operation are rejected at instantiation.
Input capability is consumed once. Reads are pull-based; one next delivers one line. LF and any CR immediately before it are removed; lone CR remains data.
UTF-8 is strictly validated when a line is assembled, rejecting incomplete terminal sequences too. BOM/NUL remain data; Unicode normalization is not performed.
A trailing LF creates no extra empty line; a remaining unterminated line is delivered once. Empty input is immediate EOF.
Accumulation byte buffers grow geometrically. The default line limit is 1 MiB, configurable from 1 byte to 8 MiB. The limit counts input bytes excluding LF but including CR.

Error codes are 1 for invalid UTF-8, 2 for read failure, 3 for write failure, 4 for excessive line length, and 5 for consumed input or aborted write.
Read failure closes the source, delivers Result.err once, then EOF. Write encodes text as UTF-8 with LF; Future completion corresponds to sink completion.
An application's returned Result.err is a normal returned task outcome, distinct from a trap.

Input return detaches in-progress next and waits for source.return. Write Symbol.asyncDispose signals AbortSignal and waits for sink completion.
Task cancellation is acknowledged after source checkpoints/unwind, owned-endpoint drops, buffer release, and async cleanup complete. Input/sink must cooperate with cleanup.
A closed reader detaches its source, current chunk, and accumulation buffer and does not retain unnecessary source.return values.

### Node CLI

`text-cli.mjs output-directory [--input file] [--output file]` calls main without arguments and receives `result<i32,i32>`. Connections default to stdin/stdout; the host lazily opens files.
`examples/io/text-filter.walz` filters out empty lines, prints the rest, and returns the line count on success. Neither count nor manifest is mixed into stdout.
Exit codes are 0 for success, 1 for Result.err or execution failure, and 130 for TaskCancelledError from SIGINT. Cleanup is awaited after SIGINT, closing files being read/written.
Input/output paths that are identical or reference the same file are rejected before writing. Explicitly selected output files are created/overwritten, including an empty file for empty output.
Node handle management is separated into runtime/node-io.mjs. Borrowed stdout is not closed on normal completion; factory-opened files are awaited through end/close. Node cancellation closes input/output streams, so they are connected to a single execution scope.

This WIT and integration code are Walz-specific, not wasi:* package implementations or a general WIT import generator. No semantic-preservation proof extending existing Lean models is added; integration is compared through Rust/Node/Chromium execution checks.

<a id="文字列ライブラリと配列のコピー"></a>

## String Library and Array Copying

The argument of `@bytes.from-array` is a non-null reference to the specified array type. Operands are evaluated once, from left to right. The result is an independent immutable snapshot; later array writes cannot change it. Range checks use unsigned start/length, requiring start <= size and length <= size-start. Only selected elements are converted. `@bytes.copy-to-array` reads immutable bytes and writes the destination; it has the writes effect and cannot expose private bytes storage.

Typed GC arrays accept standard WAT packed i8/i16 storage, with i32 values truncated on stores and explicit `array.get_s`/`array.get_u` extension on loads. Bare `array.get` remains for unpacked arrays. Standard `array.copy $Destination $Source dst at src start length` evaluates operands left to right and preserves Wasm bounds, null and overlapping-copy semantics. Its typed subset requires a mutable destination and matching numeric element/storage types; it reads mutable sources and writes the destination. Reference-array copies remain on the standard WAT validator path. Opaque representations remain private for both source and destination. Typed list/array-view conversion continues to require unpacked storage.

Fixed-width byte reads use `(@bytes.read-FORMAT-ENDIAN bytes offset)`, with FORMAT `i16/u16/i32/i64/f32/f64` and ENDIAN `le/be`. Imported `$bytes.read-*` and `$bytes-view.read-*` functions provide the same operations; views enforce their own logical range. Reads return i32 for i16/u16/i32, otherwise the named Wasm numeric type. i16 sign-extends, u16 zero-extends. `(@array.write-FORMAT-ENDIAN $Array destination offset value)` supports i16/i32/i64/f32/f64 on non-null named mutable packed i8 arrays and returns unit. i16 writes truncate to 16 bits. Floating-point accesses reinterpret bits, including signed zero and NaN payloads within Wasm; JS number boundaries need not preserve NaN payloads. Operands evaluate once from left to right, followed by unsigned range checks (`offset <= size`, `width <= size-offset`). Unaligned accesses are allowed. Incomplete ranges trap, and writes validate the entire range before mutation. Reads are pure; writes have the writes effect and cannot bypass opaque storage or ownership checks. Current lowering uses GC accesses and shifts, without linear scratch memory. See [fixed-width byte access](docs/byte-access.md).

`walz:core` provides `$i32.checked-add-u`, `$i32.checked-mul-u`, and `$i32.checked-align-up-u`, with i64 counterparts. Each takes two scalars of its named width, has the pure effect, and returns Option of the same scalar. Inputs are interpreted as unsigned bit patterns, including high-bit patterns. Add/mul return none when the mathematical result exceeds the unsigned width. Alignment rounds up to a nonzero power of two and returns none for invalid alignment or overflow; the high-bit power of two is valid. A successful zero is some(0). Arguments evaluate once left to right as ordinary calls. Wasm wrapping arithmetic, numeric types, and ABI signed storage mappings are unchanged; callers check application limits separately. These are library functions with existing type/effect/ownership checks, not new syntax or unchecked arithmetic assumptions. See [checked integers](docs/checked-integers.md).
Storage is not shared with the original array, so later array changes do not alter bytes/text content.
Reading a mutable array has reads, and memory analysis retains element reads on that target. The returned value has no reference to the original array.
New bytes have no observable identity, so physical storage allocation is distinguished from semantic allocates.

Use `(@include "walz/core/text.walz")` at a root declaration position, adjusting the include path relative to the caller.
Provided functions are `$bytes.concat(bytes,bytes)`, `$string.concat(text,text)`, `$string.u64(i64)`, and `$string.s64(i64)`. Combine `$json.stringify` with `$json.from-string` for JSON-string construction/output.
U64 converts a 64-bit bit pattern to unsigned decimal; s64 converts it to signed decimal.
Json returns a quoted JSON string, escaping quotes, backslashes, and control characters.
These are ordinary Walz functions with reads/writes/allocates bounds because they construct/mutate local GC arrays.
Mutable work arrays are separated by copying from public immutable bytes/text. Concat takes time proportional to the total input length.

Search, splitting, bounded integer parsing, and text/JSON builder APIs are defined in [walz/core/README.md](walz/core/README.md).
Search positions are unsigned UTF-8 byte offsets; text search beginning inside a character returns none.
Integer parsing checks limits before multiplication and returns option.none for empty input, invalid ranges, or overflow.
Builders are ordinary opaque mutable GC types. Finish detaches fragment references and restores a reusable empty state.
JSON object builders manage escaping of field names/values and separators; nested children are finished before incorporation.
These add no built-in language types or syntax and follow existing type/effect/opaque checks. No Lean semantic-preservation proof is added.

<a id="result-と-stream-の限定マクロ"></a>

## Limited Result and Stream Macros

```wat
(@result.let ($value) (@inst $result T E) U subject body)
(@stream.fold-result T S E source initial ($state $item) body)
```

Result.let evaluates its subject once and binds an ok payload in body. The body returns result<U,E>.
Err propagates with the same E. In the initial version, E must be shareable. For T=unit, use binding `()`. E=unit is also supported.
If a success payload is owned, the existing checker requires explicit move and consumption in the body.

Stream.fold-result consumes stream<result<T,E>> with shareable, nonunit state S.
It evaluates source then initial, once each, and body for every ok payload. Body returns result<S,E>.
EOF closes the Stream and returns ok(state); an input err or body err closes the remaining Stream and returns err.
State S is nonunit. When T=unit, the binding is only `($state)`.
Body expands to loop/match within the same function without generating dynamic callbacks or closures.

Temporary names are generated to avoid every source identifier; ordinary name/type/effect/ownership checks run after expansion.
Expanded depth is limited to 256 and node count to one million. Suspension/cancellation protocols use existing task scopes.
This is not finally for arbitrary throws. Explicit abrupt exits still reject unconsumed endpoints.
A result.let err path leaving another owned value behind is also rejected.

<a id="opaque-な公開型"></a>

## Opaque Public Types

```wat
;; counter.walz
(module
  (@export (opaque $State) (func $new) (func $get))
  (type $State (struct (field $n (mut i32))))
  (func $new (result (ref $State)) (struct.new $State (i32.const 0)))
  (func $get (param $s (ref $State)) (result i32)
    (struct.get $State $n (local.get $s))))
```

External code can reference/pass the type nominally. Ordinary `(@use "./counter.walz" (type $State $Alias))` is also supported.
Construction, field/element reads/writes, variant match, array length, and bytes.from-array are allowed only within the declaring file.
The same checks apply through aliases and inferred references. Ref.eq, nullability, and GC semantics use existing rules.
Checks compare typed-IR operations with original declaration scopes, adding no runtime wrapper or ABI.
Per-field publication and friend access are not part of the initial version.

<a id="ソースの整形と-cli-の診断"></a>

## Source Formatting and CLI Diagnostics

`walz fmt file.walz` formats only that file; `fmt-check` exits with code 1 if unformatted. Include targets are not written automatically.
String escapes, line comments, and nested block comments are preserved; only whitespace between tokens changes.
Unclosed parentheses, strings, or comments prevent writes. Unclosed parentheses report their opening position.
The formatter does not replace type checking and also works with standard WAT stack syntax.
Text CLI Result err displays `program error <code>`, including application errors.
Exit codes remain 0 for normal main completion at EOF, 1 for Result err, and 130 for SIGINT.

<a id="透明な型-alias-と局所的な型引数の省略"></a>

## Transparent Type Aliases and Local Type-Argument Omission

`(@type.alias $Name T)` is transparent shorthand expanded at type occurrences. It uses the type-name namespace and cannot share a name with a GC type or closure.
Forward references are allowed; cycles, depth above 256, or over one million expanded nodes are rejected. Even unused targets are checked under existing type/ownership rules.
Aliases themselves do not become nominal runtime-IR types, and unused alias types add no runtime Wasm types.
File-level `@export (type $Name)` and `@use` are supported. An alias cannot be opaque; target opaque restrictions still apply after expansion.
Write `$Name` in type positions. It cannot be used in `(ref $Name)`, `struct.get $Name`, or other positions requiring a heap type ID.
Unit aliases are limited to payload positions; they do not introduce unit into ordinary function parameters or fields.

The following shorthands supplement existing explicit forms.

```text
(@result.ok [value])                 ;; expected result<T,E>
(@result.err [error])                ;; expected result<T,E>
(@tuple.new [first] [second])         ;; expected pair<T,U>、unit成分は省略
(@option.none) / (@option.some value)  ;; expected option<T>
(@box.new [value])                   ;; expected own<T>
(@match subject (@case variant ($names...) body)...)
(@let ($first $second) subject body)
(@result.let ($value) subject body)
(@stream.fold-result source initial ($state $item) body)
```

Value-construction shorthands are checked when an expected type is determined from a function result, call argument, explicit binding type, field, explicit branch result type, or similar context.
A bare constructor without an expected type requires an annotation. Match shorthand determines the variant/value type from its subject and requires an expected output type.
The existing rule that omitted results mean unit for functions/if/block/long match is retained. No whole-program inference solving interfunction constraints or guessing unresolved types is performed.

Pattern let evaluates its pair subject once and produces the same typed IR as the existing single-case match.
Shared-value duplication and owned-value move/consumption obligations are the same as ordinary match. Unit-component bindings are omitted; duplicate binding names are rejected.

Short result.let derives T/E from its subject and locally derives U from the expected type or body.
E must be the same shareable type; even changing between nullable and non-null is not allowed. Owned success payloads can be passed through explicit move.
Err propagation is local to the expression, not an implicit function return.
Short fold derives types from source stream<result<T,E>> and initial, expands to existing loop/match, and is checked.
S is shareable and nonunit, E is shared, and body returns result<S,E>. Source/initial evaluation order, once-only evaluation, EOF/err close, and task-cancellation checks match the existing form.
Compile-time type search adds no execution and leaves no provisional local/loop in the body. Expression checking is limited to one million steps per function.

<a id="外部-resource-と借用"></a>

## External Resources and Borrowing

Nominal resources with synchronous destructors, nonescaping borrows, and using are provided.
See [External-resource contracts](docs/resources.md) for details and ownership/exception/host-boundary restrictions.
See [WIT bindings](docs/wit-bindings.md) for WIT binding generation and supported types.

<a id="不変の-list"></a>

## Immutable Lists

See [List contract](docs/lists.md) for `(@inst $list T)`, five operations, and WIT list<T> conversion.
Owned elements and borrow storage are not permitted.

<a id="利用者定義のジェネリクス"></a>

## User-Defined Generics

`(@type.params $T ...)` and `(@inst $name T...)` monomorphize functions, types, aliases, and closures.

`$name<T, U>` is an equivalent surface shorthand in type and nominal function/heap reference positions, including nested value types, GC references, calls, construction, generic closures, and concrete declarative function exports. `<` is adjacent to the name; commas separate arguments, whitespace/comments/newlines and a trailing comma are allowed, and empty/malformed lists produce `E_GENERIC_SYNTAX`. Arguments can contain existing S-expression types; reference wrappers stay explicit. Literal names declared/imported in the namespace take precedence over shorthand, and `@inst` remains an unambiguous explicit application. Locals, binders, quoted WAT identifiers, metadata, and operators are not reinterpreted. Lowering retains source spans and uses existing type/bound/effect/ownership/opaque checks and specialization caching. The formatter and Watz view support the shorthand; partial editor syntax is never compiled.
See [Monomorphization contract](docs/generics.md) for application-time checks, recursion/expansion limits, public ABI, and map/fold.
Where immediately after `@type.params` checks shared/nonunit/callable, built-in Add/Sub/Mul/Div/Eq/Ord/Logical/Bitwise/Shift, and callable effect bounds at instantiation.
Operators resolve against fixed basic-type instances, constraints are erased, and output becomes existing closed IR. No user-defined class/impl or abstract body checking is added. See [Type classes and operators](docs/typeclasses.md).
Logical treats i32 zero/nonzero as booleans, expanding `&&`/`||` to short-circuit if and normalization to 0/1. Unexecuted right operands still undergo type/effect/ownership checks. Bitwise and Shift are fixed to i32/i64; shift counts have the same type. `>>` sign-extends, `>>>` zero-fills, and count masking follows Wasm rules.

<a id="スカラー-newtype-と構造化された早期脱出"></a>

## Scalar Newtypes and Structured Early Exit

`(type $Id (@newtype i64))`, `@newtype.wrap` / `@newtype.unwrap`, and flat `@result.do` are provided.
Standard WAT `return`, named block, `br`, and `br_if` are also checked in the typed core and run cleanup for exited `@using` scopes.
[Numeric and control rules](docs/core-expressions.md) defines types, evaluation order, ownership certificates, ABI erasure, and supported scope.

<a id="wasm-threads-と共有メモリ"></a>

## Wasm Threads and Shared Memory

Typed IR handles named memory32 definitions/imports, 67 threads atomic instructions, and memory.size/grow. `shared` requires a maximum page count. WAT instruction names/memory immediates are retained; integer addresses/payloads and natural alignment are checked.
`@effect synchronizes` bounds synchronization, and `blocks` bounds synchronous waiting. These are distinct from Future `suspends`.
Ownership checks apply existing GC/endpoint move and consumption rules; they do not prove exclusive ownership in shared regions or absence of host races.
See [Syntax, boundaries, guarantees, and examples](docs/threads.md).

<a id="固定-atomic-セルの条件付き証明"></a>

## Conditional Proofs for Fixed Atomic Cells

`(@race-free)` puts all definitions in the entry module into verification mode.
Declare fixed cells with `(@atomic.cell $counter $memory i32 0)` and use `(@atomic.at $counter)` in atomic address positions. Cell types are i32/i64; at most one fixed-size shared memory is allowed. Raw addresses, narrow accesses, offsets, size/grow, and host-function imports are rejected.
Assuming the host obeys the same cell contract, Lean proves absence of data races, agreement on widths for the same range, and in-bounds access for access sequences of arbitrary length. Compiler extraction/output and the engine are TCB.
See [Syntax, host contract, machine checking, and limitations](docs/atomic-cells.md).

<a id="公開後不変の共有領域とworkerホスト"></a>

## Shared Regions Immutable After Publication and Worker Hosts

`(@readonly.region $index $memory byte-offset positive-byte-length)` and `(@readonly.load $index u8/i32/i64/v128 relative-byte-offset)` describe dynamic reads within statically declared regions. Address expressions are evaluated once; unsigned bounds checks include load width. They have `reads` and are nonatomic, so unaligned access follows Wasm rules.

Combined with `@race-free`, version 2 certificates check readonly-access traces separated from atomic cells in Lean, assuming host immutability after publication. This mode permits additional defined unshared memories as private. The Core+JSPI ABI is v4; the shared host contract is `walz-readonly-regions-v1`. Declarations without `@race-free` add no proof of write-prohibiting ownership or freezing. [Rules and TCB](docs/readonly-regions.md).

Node/browser `WorkerHost` is an explicit host API outside the language. It handles driver initialization, job queues, individual cancellation, join, and shutdown; data formats and Wasm function names are separated into drivers. GC references and Future endpoints are not transferred to Workers implicitly. `@task.spawn` remains cooperative. [Delivery, lifetime, and cancellation contracts](docs/worker-host.md).

<a id="cli-と拡張の実行範囲"></a>

## CLI and Extension Execution Scope

Rust 1.99 is the baseline; `walz run` / `walz build --target core-js` explicitly select the host and runtime. See [CLI](docs/cli.md).
[WIT bindings](docs/wit-bindings.md) defines `@abi.type` record/enum/flags/unsigned and WIT generation scope; [Validator bridge](docs/wat-validator.md) defines numeric-memory instructions that can mix with typed macros.
`(@inst $task T)` is the owned child-task type from `walz:core`, used with `@task.spawn T $Closure environment` / `@task.join T handle` / `@task.drop T handle` / `@task.cancel T handle` / `@task.join-timeout T milliseconds handle` and `@task.scope (result T) body`. GC environment/shared-result identity and completion waiting on scope exit are preserved. [Contracts and restrictions](docs/scoped-tasks.md).

<a id="明示的な-stream-batch-と-deadline"></a>

## Explicit Stream Batches and Deadlines

`@stream.read-batch T max stream` reads with a limit of 1–65536 and returns an owned tail and immutable list.
An empty list means EOF; a nonempty list is a prefix of at most max elements. Existing single-element next behavior is unchanged.
See [ABI and partial progress](docs/stream-batches.md) and [Public-call scope deadlines](docs/scoped-tasks.md).

<a id="エディターと-native-component"></a>

## Editor and Native Components

Stdio `walz lsp` provides type/effect/ownership diagnostics, hover, and go-to-definition. [LSP](docs/lsp.md) defines UTF-16, unsaved relative modules, bounded caches, and cancellation contracts.
Component output specifying a WIT world connects shared-payload async endpoints and typed own/borrow of imported resources to the Canonical ABI. [Native worlds](docs/native-worlds.md) defines supported shapes and execution conditions.

<a id="期待型付き文字列と共有値combinator"></a>

## Expected-Type Strings and Shared-Value Combinators

In expression positions whose expected type is determined to be the semantic string type, a UTF-8 string token expands to the same typed Data expression as `@string.lit`. A missing expected type or requirement for another type yields `E_LITERAL_TYPE`. Implicit-conversion, ownership, and effect rules do not change.
`walz:core` `$option.ok-or` / `$result.map-err` are ordinary monomorphized functions with shared/nonunit constraints. They follow [Argument evaluation order and callback contracts](docs/option-result.md).

## Optional printed view

Watz match clauses use `case`/`default`; canonical Walz keeps `@case`/`@default`.

Walz S-expressions are the default source and output of `walz print`. The optional `--format watz` view preserves the same source forms using `func`, `@export("name")`, and direct `match`/`if` expressions; saved `.watz` views can also be read back. The module wrapper is optional and immutable bindings use `let`. It lowers before ordinary module resolution and checking; it adds no types or runtime semantics. The canonical language remains WAT S-expressions. See [the complete mapping](docs/review-syntax.md) for syntax, escapes, limits, comments, and tooling boundaries.

## Component composition source

A root `(component ...)` builds a Component, while `(module ...)` retains Core
semantics. `(@lib declarations...)` is a static library with no executable ABI,
start, global/data initialization, or Core exports; it can be checked and selected
with ordinary static `@use`/`@export`. A standalone library is not a build entry.

Inside a Component, `(@expose "public-name" $function<concrete-types...>)`
monomorphizes and checks the implementation, then infers a portable Canonical ABI
export. `(@connect "module" "member" $component-function)` supplies a Walz import
with the selected Component function. Actual parameter names and async status
come from that function; final Component validation checks type compatibility.
It retains a host effect/capability. Standard Component definitions and explicit
instantiation order are preserved. Selecting `(@use "./unit.wasm" (component
$Constructor))`, also supported for Component `.walz` sources, loads a definition
without allocating runtime state. Instance identity is chosen by explicit
`instantiate`, not by the filesystem path.

Source constructors accept `(@type.params $T...)` after their name and optional
`@where` constraints. `(instantiate $Constructor<i32>)` and
`(instantiate (@inst $Constructor i32))` explicitly specialize them. Arguments
resolve in the caller's static type scope to closed value types or visible nominal
type definitions;
standard types still require explicit imports. Aliases normalize before caching.
The same specialization shares a definition, while each instantiate creates fresh
state and defined resource identities. Type-only `@lib`/`@use` declarations need
no exposed function and emit no runtime implementation. Type substitution keeps
term binders and nested type binders separate. Concrete bodies retain all normal
type, bound, effect, ownership, proof and Wasm checks. Binaries are concrete and
do not accept type arguments. Generic scopes require named Component references
and named Component outer aliases so generated definitions cannot redirect numeric
indices; ordinary native WAT and Core instruction indices retain their semantics.
Nominal snapshots preserve recursive layouts, mutability, packing, nullability
and ABI annotations. Source declarations and phantom generic applications retain
distinct cache identities. Opaque layouts, closures and resource/borrow identities
cannot be copied. Public named values require native type ports; exported instance
ascriptions freshen value naming aliases and pass Component subtyping without
changing resource identity or adding runtime conversions. Internal constructors
may lack a standalone WIT projection; the final `--wit` entry must have one.

`(@world "namespace:package@version" "world")` names the interface decoded from
the validated binary. `build --wit` writes `world.wit` beside the output;
`--wit-input FILE [--world NAME]` consumes a separately declared contract. The
latter builds native worlds from modules or validates the exact public ports and
types of a Component. Core-module exports can have no WIT representation and are
still valid standard Component outputs unless WIT is requested.

`(@component.type "binding" (param "function" index-or-name))`, `(result "function")`
and `(type "member")` project value types from explicit connections. Nested
carrier/field/case/tuple projections keep WIT ABI annotations and actual resource
identity. Implementation queries run before checked elaboration. Constructor
arguments can query preceding validated native connections, including earlier
specializations; binding declarations themselves may appear later. Only reachable
queries participate in a stage, and constraints see the real projected types.
No initializer or host function runs during inference. Forward/cyclic dependencies
report `E_COMPONENT_TYPE_PHASE`; discovery witnesses never enter implementation IR.
No host import, destructor or capability is inferred from a query. Named values
keep externally named type ports; resources require explicit source declarations.
Unknown selectors and wrong contexts are errors, while opaque annotations stay data.
See [connected type queries](docs/component-composition.md#connected-type-queries).

Generated adapter loads/stores name their ABI memory. Source memories retain
their own references; canonical operations select a particular memory context.
Memory identities are per Core instance, with imported aliases sharing identity
and fresh defined memories per instantiation. Inspection reports unresolved host
or alias cases; it is not an optimizer or race-freedom certificate. Endpoint
buffers live until completion/acknowledgement, not just cancellation request.
Native traps/cancellation follow instance/store lifetime; SDK cleanup guarantees
do not apply to arbitrary native hosts.

Portable value inference includes supported shared data and Future/Stream
endpoints. `(@connect "namespace" (instance $target))` additionally infers imported
resource contracts for `@expose`: source declarations retain explicit own/borrow,
moves and destructor imports, which map to canonical drop. Imported type aliases
are resolved by validator identity; unrelated resource definitions never unify by
representation or spelling. Only explicitly declared imports can be called.
Instance and function bindings cannot overlap one namespace. Supported owned
Option/Result/pair carriers and monomorphized owned functions preserve the same
checks. Raw GC references, v128, externref and open generics are excluded; owned
list/record/endpoint payloads remain unsupported. Constructor type arguments
copy type definitions rather than heap objects between GC agents; opaque layouts
and resource/borrow identities remain outside type arguments. Vectors, boxes and task types can specialize private implementation code;
public signatures still require portable WIT types. Explicit native WIT module adapters retain resource own/borrow support. `walz:ffi-v1` custom metadata is allowed only inside
Core modules, with strict version-1 JSON function descriptions that match actual
exported binary signatures, including externref. It grants no capability or
ownership assumption and does not make externref portable. `walz:world-v1` is
reserved for checked world metadata; unrelated custom sections remain opaque.
See [Component composition](docs/component-composition.md) for exact metadata,
examples, runtime boundaries and validation tasks.
