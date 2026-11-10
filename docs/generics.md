# Type Arguments and Monomorphization

Place `(@type.params $T ...)` immediately after a declaration name. The syntax is shared by `func`, `type`, `@type.alias`,
and `@closure`; explicit application uses `(@inst $name T...)`. Ordinary function calls
can omit type arguments uniquely determined by the arguments and expected type. This introduces no user-defined traits, dictionary passing, or runtime type erasure. Ordinary WAT functions and calls remain available unchanged.

```wat
(module
  (func $identity (@type.params $T) (param $x $T) (result $T) (@effect pure)
    (local.get $x))
  (func $main (export "main") (result i64) (@effect pure)
    (call (@inst $identity i64) (i64.const 42))))
```

```wat
(@type.alias $Values (@type.params $T) (@inst $list $T))
(type $Box (@type.params $T) (struct (field $T)))
(@closure $Identity (@type.params $T) (param $x $T) (result $T) (local.get $x))

;; 値型、heap 型、呼び出し先にも同じ適用形を使う。
(param $xs (@inst $Values i64))
(param $box (ref (@inst $Box i64)))
(struct.new (@inst $Box i64) (i64.const 42))
(@closure.call (@inst $Identity i64) (@closure.new (@inst $Identity i64)) (i64.const 42))
```

Type arguments can be numeric types, value types, GC references, or concrete nominal type names. When passing a nominal name,
use it in positions that require that name, such as `(ref $F)` or `closure.call $F`. A bare heap name
cannot appear where a value type is required. Unit also follows the existing rule restricting it to payload positions.
Type and closure construction requires all arguments explicitly. Unapplied generic declarations cannot be passed as type or function values.

The same declaration and concrete type-argument tuple shares one instantiation throughout the module.
Transparent aliases expand before argument comparison. Nominal types and distinct closure types retain their identities.
Import aliases and re-exports of the same file do not duplicate instantiations.
Substitution changes only type positions and nominal references, leaving same-named locals, bindings, and labels unchanged.
Instantiations of opaque types also retain the restrictions of the file that defined the original type.

Existing type, effect, ownership, and borrow checks run after instantiation, followed by optimization and DCE.
Applications inside unused ordinary functions are checked too. Generic template declaration names, type parameters, and
referenced names are checked at declaration time; body types, effects, and ownership are checked for each instantiation.
An unapplied template's body has no guarantee of correctness for arbitrary type arguments.
Errors include both the body location and the site requesting instantiation.

Recursive functions and recursive GC types with unchanged type arguments are supported. Polymorphic recursion that
changes type arguments during recursion and requests a new instantiation is rejected. Instantiations are queued, with limits of 1,024 declarations,
64 dependency levels, and 1,000,000 expanded nodes. Inference type matching also has limits of depth 64 and 1,000,000 type-node visits;
whole-module rechecking with instantiation has a cumulative limit of 2,000,000 node visits. Exceeding a limit produces a compile diagnostic.

Generic declarations cannot have direct Wasm exports. Export a concrete wrapper, or write
`(export "identity" (func (@inst $identity i64)))`.
Only concrete types appear in WIT and both JS ABI signatures. Host imports also require concrete types.

## Inference for Ordinary Function Calls

```wat
(func $id (@type.params $T) (param $x $T) (result $T) (@effect pure) $x)
(func $answer (result i64) (@effect pure) (call $id 42))
;; 結果の期待型 i64 により T=i64、裸の 42 も i64。
;; (call $id (i32.const 42)) は引数から T=i32。
```

Inference covers only type arguments of `call $generic`. Function signatures remain declared; no global constraint solving occurs.
Arguments and expected result types are matched structurally, including transparent aliases, list/option/result/tuple/box, and nominal type applications.
`callable` constraints also supply equalities with a known closure's parameter/result types, so `(call $list.map xs closure)` determines the output element type too.
The same bound, effect, ownership, and race checks as explicit application then run.

Unresolved type arguments, conflicting types, or undetermined widths such as `(call $id 1)` produce `E_GENERIC_INFERENCE`.
Specify them with `(@inst $id i64)` or a type annotation. There are no default numeric widths, implicit numeric conversions, guessed phantom type arguments,
or synthesized types merging distinct nominal types. Type and closure applications and export instantiations continue to use `@inst`.
Explicit and inferred applications with the same type tuple share an instantiation rather than instantiating separately per call site.

This inference also works for ordinary `call` inside macros. A macro is unnecessary merely to fill in type arguments; use [user-defined macros](user-macros.md) when binding names or rearranging syntax. Public `infer` selectors read information such as element types; they do not register new typing rules.

Ordinary macro expansion resumes checking from the affected expression, but an instantiation that has not yet been generated rebuilds the type environment and rechecks the whole module. Temporary locals created during inference and checked types from older revisions are not reused. Type, effect, and ownership contracts are checked after instantiation as well.

## Map/Fold with Concrete Closure Types

`walz/core/list.walz` exports the following generic functions through named imports.

```wat
(@use "walz:core" (func $list.map) (func $list.fold))
(call (@inst $list.map i64 $string $Label) xs label-closure)
(call (@inst $list.fold i64 i64 $Sum) xs sum-closure initial)
```

Map uses shareable, non-unit T/U; fold uses shareable, non-unit T/S. Both use a concrete,
repeatedly callable closure type F. Passing incompatible types or contracts is rejected during instantiation checking.
An empty map never invokes its callback; otherwise each element is visited once from the start.
An empty fold returns its initial value. GC arrays are mutated only inside map, then copied into the returned list.
No indirect calls or additional callback runtime are introduced.
Callback effects are also derived from the closed call graph after instantiation.

See [generic-lists.walz](../examples/generic-lists.walz) for an executable example.
`just test-generics` checks optimized and unoptimized execution, Wasmtime, both JS ABIs, memory reclamation,
instantiation sharing, ownership, opaque types, name collisions, and expansion limits.

## Type-Argument Bounds at Instantiation

One optional `(@where constraint...)` can follow `@type.params` immediately.
This is shared by func/type/type.alias/closure, and every constraint must hold.
Constraint targets explicitly name type parameters of that declaration.

```wat
(func $map-pure (@type.params $T $U $F)
  (@where
    (shared $T $U)
    (nonunit $T $U)
    (callable $F (param $T) (result $U) (@effect pure)))
  (param $xs (@inst $list $T)) (param $f (ref $F)) (result (@inst $list $U))
  (call (@inst $list.map $T $U $F) (local.get $xs) (local.get $f)))
```

| Constraint | Check |
| --- | --- |
| `(shared $T...)` | A shareable type containing neither owned values nor borrows. Recursively checks GC types, value types, and closure captures |
| `(nonunit $T...)` | A type other than unit. Ordinary type checking also checks whether the type is valid for storage or parameters |
| `(Add $T...)` / `(Eq $T...)` / `(Ord $T...)` | A concrete type with a fixed instance of the [built-in typeclass](typeclasses.md) |
| `(Sub $T...)` / `(Mul $T...)` / `(Div $T...)` | The same four scalar types as addition. Integer division is Wasm signed division |
| `(Logical $T...)` / `(Bitwise $T...)` / `(Shift $T...)` | Logical operations use i32; bitwise operations/shifts use i32/i64. Logical operations expand into short-circuiting if expressions |
| `(callable $F (param T...) (result U) [(@effect ...)])` | F is a concrete, shareable, repeatedly callable nominal closure type with exactly matching parameter and result types |

Shared does not require immutability. References to mutable GC arrays/cells are allowed if their contents are shareable.
Callable signatures compare concrete types strictly after expanding aliases, without introducing argument contravariance or similar rules.
`(param)` means no parameters; `(result $unit)` means a unit result. A one-shot closure with owned
captures does not satisfy callable. Explicit owned types in parameters or results are themselves allowed.

Callable without `@effect` imposes no effect bound. If specified, it uses the same
reads/writes/allocates/suspends/host, `(throws $Tag...)`, and `(throws *)` as ordinary effect contracts.
Pure must appear alone. Effects are inferred from the actual body, including recursion, callees, and catches;
the function's own effect declaration is not treated as evidence. Host imports follow the trusted policy,
conservatively treating effects as unknown when unspecified.

Types and callable signatures are checked once the instantiated declaration headers are available, before body type checking.
Effect bounds are checked after body type checking and host-policy application.
Both precede DCE/optimization and include uncalled callbacks and applications inside unused ordinary functions.
`E_GENERIC_BOUND` reports both the application and constraint locations; effect violations include a call path.

Even without application, where-clause placement, structure, target parameter names, and effect grammar are checked.
No declaration-time body checking with abstract type variables, user-defined typeclasses, instance search, or dictionary passing is added.
Constraints are erased as checking information and add no executable functions or runtime state.
Map/fold in walz/core/list.walz also declares shared/nonunit/callable.
The example's `scale-pure` exports map restricted to pure callbacks through both ABIs.
