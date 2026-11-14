# Bootstrap modular frontend

The compiler written in Walz implements static modules, fixed-form lowering,
GC effect bounds, a minimal core library, monomorphization, declarative user
macros and checked record/tuple ABI views. This bounded compiler compiles its own
sources into matching Core stages. Rust still builds the seed and its Core/JSPI
or Component adapter; broader feature parity remains incomplete. See
[self-compiled stages](selfhost-stages.md).

See [the lower-level compiler contracts](selfhost.md) for parsing, raw IR,
independent validation, binary encoding and diagnostics. This frontend runs in
Walz; the host supplies files and transfers bytes. No Rust or JavaScript compiler
callback participates in name resolution, expansion or specialization.

## Try a project

```sh
just build-selfhost
just selfhost-check examples/selfhost/project/main.walz
just selfhost-compile examples/selfhost/project/main.walz target/selfhost-project.wasm
node --input-type=module -e 'import {readFile} from "node:fs/promises"; const {instance} = await WebAssembly.instantiate(await readFile("target/selfhost-project.wasm")); console.log(instance.exports.main());'
# 42
```

The [entry file](../examples/selfhost/project/main.walz) imports a generic Box
from [box.walz](../examples/selfhost/project/box.walz), constructs a list,
iterates over it, updates an explicit mutable GC field and matches a Result. Its imported
[macro](../examples/selfhost/project/syntax.walz) infers the argument type, checks
an Add guard and evaluates its argument once.
The same source runs with the Rust CLI:

```sh
walz run examples/selfhost/project/main.walz
# 42
```

## API and validation boundary

`examples/selfhost/portable.walz` exposes five operations. JS names use camelCase;
Walz source names retain hyphens.

| JS operation | Input | Success value | Error |
| --- | --- | --- | --- |
| `compile(source)` | bytes | Core Wasm bytes | `{code, start, end}` |
| `check(source)` | bytes | emitted function count | `{code, start, end}` |
| `compileProject(files)` | source-file list | Core Wasm bytes | `{file, code, start, end}` |
| `checkProject(files)` | source-file list | emitted function count | `{file, code, start, end}` |
| `imports(source)` | bytes | dependency descriptors | `{file, code, start, end}` |

Each operation returns an explicit Result. A source-file is
`{path: string, source: bytes}`. The first file is the entry; other records supply
dependencies. `imports` returns records with the requested path and empty source
bytes. It discovers dependencies for host I/O; it does not authorize a path,
resolve names or certify a module. The CLI reads relative files and asks the
guest to resolve and check the complete project.

Error `file` is the supplied file's zero-based index; `-1` identifies the embedded
`walz:core`. Ranges are half-open UTF-8 byte offsets in the original file.
Synthetic nodes retain their source origin. A final result/stack mismatch can
point to the whole function rather than a particular literal.

```text
files → parse / generic spelling normalization
      → static linking and opaque access checks
      → checked source ABI views and retained layout facts
      → specialization and typed core expansion
      → lexical/control lowering and GC recursion-group ordering
      → bounded postorder syntax + origin map
      → independent type/stack/effect validation
      → opaque checked module → binary emitter
```

The frontend's public immutable trees and expanded text are untrusted data.
Generated syntax is assembled directly, without text serialization or reparsing;
the optional expansion/debug printer retains its separate text limit.
Only the independent validator creates the opaque module accepted by the emitter.
Every emitted instruction is checked, including unreachable code. Fixed-form
expansion is not a substitute for type, effect, ownership or proof checking.
Unsupported contracts are rejected rather than silently removed.

Supported `@abi.type` record/tuple annotations receive a separate opaque
certificate before specialization. Only those exact checked nodes may be removed
from Core syntax; immutable shape/layout facts remain available. This does not
add WIT or host adapters, or bypass instruction validation. See
[checked ABI views](selfhost-abi.md) for supported fields, opacity and budgets.

Ordinary folded WAT submitted through `compile`/`check` keeps the original
lower-level path and its canonical-output contract. Extended source and all
project API calls use this frontend. Their output is checked by independent
validators and execution; it is not promised to be byte-identical to Rust's
high-level lowering.

## 1. Static modules

A source library can use `(@lib ...)` with selected `@export` declarations. It
has no executable entry. The shared source preflight runs before dependency
I/O and before static linking. It rejects native exports (including inline
function/memory/table exports), `start`, `global`, and `data` declarations in a
library even when a declaration would be unused. A module entry selects its
source API with `@use`; no instance is created by that selection. Existing
`(module ...)` source libraries remain supported.

```wat
;; answer.walz
(@lib
  (@export (func $answer))
  (func $answer (result i32) (i32.const 42)))
```

The entry selects it with `(@use "./answer.walz" (func $answer))`.
Native `import` remains an external instantiation requirement; the bootstrap
Core emitter does not yet implement those requirements.

Component names follow the main compiler: `@connect` supplies a connection and
`@bind` is retired. Component composition and constructor selection require the
main compiler. The bootstrap implementation reports these unsupported features
before dependency discovery rather than loading a `.wasm` file as source or
creating an implicit instance. Direct function/type selection from `.wasm` and
the retired `@component.binary` descriptor are rejected. The CLI therefore
reports a guest source diagnostic even if that binary file does not exist.
`walz wit-bindgen` belongs to the main CLI; the bootstrap CLI remains a
Core-only compile/check driver.


```wat
;; lib.walz
(module
  (@export (opaque $Token) (func $make) (func $value))
  (type $Token (struct (field $value i32)))
  (func $make (result (ref $Token)) (struct.new $Token 42))
  (func $value (param $x (ref $Token)) (result i32)
    (struct.get $Token $value $x)))
```

```wat
;; main.walz
(module
  (@use "./lib.walz" (type $Token) (func $make) (func $value))
  (func (export "main") (result i32) (call $value (call $make))))
```

- Only explicitly selected type/function exports enter the importing scope.
  Private names stay private. Dependency functions are emitted when reachable;
  unused dependency functions do not require code generation for their bodies.
- Source `@export` declares a module API. Native WAT `export` declares the output
  Wasm API. Native exports on dependencies are not automatically reexported.
- File paths use `./` or `../`; lexical normalization handles `.` and `..`.
  Paths cannot escape their lexical root. The CLI uses absolute lexical labels,
  allowing a dependency in a parent directory. Identity is lexical, not a
  filesystem realpath or symlink identity.
- `walz:core` is the only builtin module name. A project cannot replace it.
  Arbitrary package namespaces and wildcard imports are not implemented.
  Types/functions/macros support selected imports, aliases and explicit
  reexports. Aliases preserve definition identity and opaque access. Cycles
  and duplicate file identities are errors.
- Numeric function/type indices are resolved in their original file before
  linking. Imported source aliases do not occupy native index slots. Hidden
  generated locals likewise do not change source local indices.
- Fresh names avoid source identifiers, including strings used as targets.
  Generic type formals and value/field binders use separate namespaces.

Opaque values may cross function boundaries. Their defining file alone may
construct, inspect or mutate their representation. Access checks run again after
specialization, so a generic cast cannot reveal a foreign opaque type. Core
representations are sealed too. Source casts/tests are conservatively rejected
when the instantiated type set contains an opaque representation inaccessible
from that source file; even an otherwise local cast can therefore be rejected.

**Conservative opacity restriction:** the modular frontend rejects declarations
and type arguments using the abstract heaps `any`, `eq`, `struct` or `array`.
Concrete GC references, `i31` and `none` remain available. This prevents erasing
an opaque type and recovering its structure through a compatible shadow type.
Ordinary folded WAT on the single-file lower-level path retains its existing
abstract internal-reference subset. Full recursive type equivalence is still
not implemented.

## 2. Fixed-form lowering

Multi-binding let is sequential and lexical: later initializers see earlier
bindings, and repeated names shadow earlier bindings.

```wat
(@let (($a (i32.const 20)) ($b (+ $a 2))) (+ $a $b))
```

`@loop` gives each binding an explicit type. Initializers execute once in the
caller's scope. Tail `@recur` evaluates arguments once, left to right, before
simultaneously replacing the loop state. It must target the innermost active
loop with the correct arity. Assigning a loop/let binding with `local.set` is an
error. Non-null references use nullable physical storage with dominated,
checked non-null reads. The independent checker validates each initializer
against the declared type before storing it. Non-null loop initializers retain
a typed temporary, and recur arguments already use typed temporaries. A nullable
value cannot satisfy a non-null annotation unless the source explicitly narrows
it. Initializer lowering adds no control frame, preserving numeric branch depth.

```wat
(@loop $sum
  (bind $n i32 6) (bind $acc i32 0) (result i32)
  (if (result i32) (i32.eqz $n)
    (then $acc)
    (else (@recur $sum (- $n 1) (+ $acc 7)))))
```

`@for` lowers its bind/while/step/body to this control structure. `@for-each`
captures a core list once, iterates in order and requires a unit body. It supports
a name or a two-element tuple pattern. Tuple lets evaluate their initializer
once. Generated slots and labels cannot be named through source indices.
`&&` and `||` short-circuit and produce i32 zero/one; extra operands are rejected.

These built-in forms form a closed set. Declarative user macros can compose them
without changing the reader or independent validation rules; see below.

## Named struct construction

`@struct.new` supports named field pairs, and `@$Type::{...}` supports curly
field groups. Generic type applications are supported. Initializers run exactly
once in written order; the resulting struct uses declaration field order.
Each field must be named and supplied exactly once. Labels do not bind locals.

```wat
(type $Pair (struct (field $left i32) (field $right i64)))
(@struct.new $Pair ($right 22) ($left 20))
(@$Pair::{:right 22 :left 20})
```

Field types provide expected types for numeric/string shorthand. The ordinary
independent validator checks the expanded code, including unreachable code;
there are no implicit numeric conversions or opaque representation grants.
Named construction charges the frontend work budget and allows at most 512
fields before that budget is checked. Standalone/unrecognized curly groups
remain errors rather than disappearing during rendering. String contents and
quoted heads cannot request constructor expansion.

## 3. Effect bounds

`@effect` accepts `pure`, or a set drawn from `reads writes allocates`.
Mutable-field/element reads contribute `reads`, writes contribute `writes`, and
construction of mutable GC storage contributes `allocates`. Immutable GC
construction and reads remain semantically pure; physical GC allocation can
still occur. Local mutation alone exposes no external effect.

The checker unions all instruction effects and propagates direct-call effects
to a fixed point, including unannotated callees, recursion and unreachable
instructions. An annotation is an upper bound; a missing annotation supplies no
bound. Unknown or duplicate effect names are rejected. A violation is code 123.
This subset has no host imports, async/exception effects or effect polymorphism.
Ownership/resource/proof constructs remain unsupported until their checks exist.

## 4. Minimal core

Explicitly select types/functions from `walz:core`. Constructors need an expected
type when their payload types cannot otherwise be inferred. Bare numbers have
no default integer width.

| Data | Supported operations |
| --- | --- |
| `$unit` | Unit function results and `shared` generic arguments |
| `$bytes` | new, len/get, slice, checked array snapshot, copy to mutable i8 array, UTF-8 validation |
| `$string` | UTF-8 literals, bytes, checked from-bytes returning Option |
| `$list<T>` | new, len/get, immutable array snapshot, ordered for-each |
| `$tuple<A,B>` | new, typed/inferred two-name let and for-each patterns |
| `$option<T>` | some/none and exhaustive match/default |
| `$result<T,E>` | ok/err and exhaustive match/default |
| Builders | list-builder and bytes-builder, including immutable publication |

`@bytes.from-array $Array value` and its optional `start length` form accept
i8, i16 and i32 array storage, mutable or immutable. Packed values are read
unsigned; i16/i32 values in the selected range must be 0..255 or execution traps.
Values outside that range are not inspected, including an empty range at the
end. Source, start and length execute once in written order before bounds/value
checks. Offset/length checks use unsigned arithmetic and subtraction to avoid
addition overflow. The result is a copied immutable snapshot, unaffected by
later source writes. Mutable source reads contribute `reads`; reading an
immutable source is semantically pure. `@bytes.copy-to-array` still requires a
mutable i8 destination. Opaque layouts and nullable source values remain errors.

Option/Result/Tuple/List payloads are currently shared and nonunit. Unit-valued
container payloads, owned resources, futures and streams are not ported.
Match checks variant/payload arities and exhaustiveness; only the selected
branch executes, and its scrutinee is evaluated once. Arbitrary user enums and field destructuring remain future work.
Named construction is supported as described below.

Bytes contain values in 0..255. Out-of-range values and invalid indexing/ranges
trap according to the operation's contract. From-array/copy-to-array require
byte storage `i8`; snapshots never retain a mutable backing array. The ranged
from-array form evaluates array, offset and length once and validates the
entire range before copying. UTF-8 decoding rejects overlong, surrogate,
truncated and out-of-range sequences; `string.from-bytes` returns none on invalid
input.

The embedded bootstrap core uses immutable balanced GC trees: construction is
O(n), lookup O(log n). Marker fields preserve specialization distinctions under
Wasm GC canonical type equivalence. This representation differs from the Rust
compiler's production core. It introduces no output host runtime, but its
allocation and lookup costs require fresh measurement. No speed or size gain is
claimed for this frontend.

`bootstrap-core.sexp` is maintained source data, not a standalone program for the
Rust frontend. `core-source.walz` embeds it in bounded literal chunks:

```sh
just selfhost-core-source
just test-selfhost-front
```

The JS embedding tool only prepares the compiler's resource at development time;
it never compiles user programs at runtime.

## 5. Monomorphization

Type and function templates use `@type.params`; use `$Name<T,...>` or canonical
`(@inst $Name T ...)` for explicit application. Arguments can also be inferred
from typed call arguments or an expected result when unambiguous.

```wat
(func $id (@type.params $T) (@where (shared $T) (nonunit $T))
  (param $x $T) (result $T) (@effect pure) $x)
;; In a context expecting i32:
(call $id<i32> 42)
```

Closed instances are cached by declaration identity and concrete type arguments.
Repeated calls share code; recursion reuses a pending instance rather than
expanding it forever. Type aliases, generic GC types and explicit recursive
groups are supported. All members of a generic explicit group must have matching
arity. Original group membership and order are retained; dependency ordering
never merges an explicit group with unrelated inferred recursive types.

`shared`, `nonunit` and the built-in scalar classes are supported constraints. `shared` permits unit;
`nonunit` excludes it. Unknown, open, malformed or wrong-arity arguments are
errors even when a parameter is unused. Add/Sub/Mul/Div/Eq/Ord/Bitwise/Shift accept i32/i64; Logical accepts i32.
The same scalar rules serve macro guards. Callable constraints and types outside
this backend remain unsupported. Generic source API exports are permitted; a native Wasm export
must have a closed signature and cannot export an unspecialized template.

## Declarative user macros

The bootstrap frontend accepts the existing [expression macro syntax](user-macros.md).
Macro declarations disappear before executable IR. The host does not evaluate
macro code or resolve generated imports.

```wat
(module
  (@macro $twice
    (rule (?value:expr)
      (infer (?T ?value))
      (when (Add ?value))
      (@let $saved (type ?T) ?value (+ $saved $saved))))
  (func $main (export "main") (result i32)
    (@effect pure)
    (@let $saved (type i32) 21 (@$twice $saved))))
```

Patterns use `expr`, `ident`, `type` and `literal` fragments, fixed syntax and
one level of trailing `...`. Literal fragments cover this backend's integer and
WAT string subset. Rules are tried in declaration order. Templates
support trailing repetitions with equal capture lengths and recursive macro
calls. A fragment is syntax: discarded arguments do not execute and need not
have a value type unless a query requests it. A template that duplicates a
capture duplicates evaluation; a let is necessary for single evaluation.

Template locals and labels receive fresh identities. Captured expressions keep
their caller's lexical scope; template function/type references use the defining
file, including private helpers. Caller-supplied bindings require `(bind ?name
?body)`. Only that complete body capture sees the new names; initializers keep
the caller's scope. Repeated bindings and delegation are supported. Every
syntactically matching delegated rule must agree on its binding interface. The
outer rule must declare the same binder/body relationship, forward the body
unchanged and introduce each supplied name once. A reconstructed delegated body,
undeclared binding or scope escape is an error.

`infer` supports an expression, the invocation's expected type, an explicit
`expected` hint, and structural selectors. Supported selectors are
`option.element`, `list.element`, `result.ok`, `result.err`, `tuple.first`,
`tuple.second`, `array.element` and `ref.heap`; selectors for unavailable core
constructors are rejected. Array queries respect opaque boundaries. `when`
compares a concrete type or tests the existing sealed scalar classes. In this
backend those classes apply to i32/i64; Logical applies to i32. There is no
implicit numeric width, bottom-type reification, mutable compiler-state access
or arbitrary macro execution. Calls in generic functions expand per concrete
specialization, including captured type arguments.

Queries elaborate the captured expression in its lexical environment, recreate
parameter/local indices and immutable lexical bindings in temporary probe
functions, then run the ordinary independent type/stack/effect validator. Every
successful compilation validates these probes, including those from rejected
guards: a fallback cannot hide an ill-typed queried expression. These functions are neither executed nor emitted. This additional
validation pass has a compilation cost; no performance improvement is claimed.
The actual expansion still passes ordinary validation, including effect bounds
and opaque access. Unsupported ownership/proof forms do not become valid through
macro expansion.

The implementation separates pattern/clause contracts (`macro-patterns.walz`),
per-compilation state/budgets (`macro-context.walz`), substitution/hygiene/delegation
(`macros.walz`) and independent query validation (`macro-validation.walz`).
Generics supplies contextual type inference. Private tree kinds 9/10 retain
capture/forwarding metadata during expansion; they are not reader syntax or a
validation certificate. All wrappers disappear before the independent validator.
Generated nodes retain template locations; captures retain original byte ranges.
The bootstrap API returns numeric source diagnostics rather than the Rust CLI's
expansion-history/TypeInfo reports. Existing Rust editor support is unchanged.

## Limits, tests and remaining work

The frontend bounds total user source at 1 MiB, projects at 64 files, generated
GC types at 1,024, sparse type-dependency edges at 65,536, cached definition
instances (functions, heap types and aliases) at 3,072, elaboration work at
1,048,576 charged visits, nested
type/expression/lowering traversal at 128, generic-angle
parsing at 64. Generated syntax is bounded at 262,144 nodes, 1,048,576 logical
visits (including shared atom hits) and 8 MiB of logical
storage (decoded values plus four bytes per child index). Debug rendering remains
bounded at 1 MiB; ordinary compilation and macro queries do not render.
The independent parser/checker
also impose their [existing limits](selfhost.md#resource-limits-and-lifetime).
Macros additionally have limits of 64 rules per definition, 512 invocations,
64 nested invocations and 100,000 charged substitution/hygiene visits. Existing
128-level syntax/traversal and generated-syntax limits still apply. Query probes
also consume the independent validator's function/instruction limits. These
bounds can reject programs accepted by the Rust compiler.
Excessive specialization returns a diagnostic instead of unbounded expansion.
These budgets do not constitute a wall-clock or memory-complexity proof.

```sh
just test-selfhost-front
just test-selfhost
just test-selfhost-e2e
just test-selfhost-bench
```

The maintained frontend correctness corpus is v10; it includes static-library,
Component phase-contract, named-construction, alias, constraint and expansion-
budget cases, checked ABI metadata and control/inference regressions, without
changing recorded benchmark inputs.
Tests cover both JS adapters and Chromium, the native project example, malformed
forms, lazy branches, source identity/ranges, opacity after specialization,
explicit recursive groups, generic caching/growth limits, effect unions,
snapshots, macro hygiene/repetition/delegation, type-query errors and ABI cleanup
after success/errors and forced GC. The phase-contract tests also check all
five entry points, original UTF-8 ranges, dependency discovery before host I/O,
and rejected library initializers. Emitted modules
also pass independent Wasm validation. ABI counters do not prove reclamation of
every guest GC allocation.

Remaining work includes ownership/proof and ABI
lowering, closures/callable constraints, the rest of core, floating point/SIMD,
multi-value control, memories and imports, and complete recursive type
equivalence. WIT/component imports and async runtimes are not output features of
this backend. The current subset passes stage 1/2 reproducibility; this does not
make it a replacement for the full Rust compiler.
Historical benchmark and browser-size captures predate GC, this
frontend and user macros; preserve them and create a new capture before comparing performance.
The remaining parity work is tracked in [Rust / bootstrap parity](selfhost-parity.md).

### Bounded expansion regressions

A cache entry identifies a declaration plus normalized type arguments. Calls do
not each consume an entry: the corpus checks 300 distinct function definitions,
1,100 uses of a single generic specialization, and rejection past the finite
the aggregate cache limit. Function and GC type pools retain their independent
1,024-entry limits; the aggregate cache now permits 3,072 function/heap/alias
instances. A separate alias-heavy regression checks that finite aggregate limit.

Operandless `(return)` is valid in unit functions and never recursively expands
itself. Extra operands are rejected; code after return still undergoes independent
type and stack validation.

Chains of identical `&&` or `||` operators become a flat result block with
conditional branches. This avoids multiplying source nesting into nested `if`
expressions. Evaluation remains left to right, each operand runs at most once,
short-circuited operands are not executed, and results are normalized to 0 or 1.
Every operand is type checked, including skipped operands. Mixed operators keep
their original grouping. The regression corpus checks 48-link chains, traps,
side-effect order, malformed arity and skipped type errors with both adapters.

The maintained production `bytes.concat` source compiles and executes through
this backend. Cached concrete layouts/field types and sparse iterative GC
ordering let the complete compiler probe pass expansion and ordering. Direct
syntax assembly removes intermediate WAT from compilation and macro queries.
The v9 probe validated source ABI metadata and created 747 function and 398
heap instances with 658,586 charged elaboration visits. Syntax assembly completes
with 248,369 nodes. Its next rejection was code 118 in independent GC type
validation, attributed to a match in `walz/frontend/modules.walz`. The v10
control/inference fixes remove that boundary and enable self-compilation and
stage 1/2 equality for this subset. See [self-compiled stages](selfhost-stages.md), [checked ABI views](selfhost-abi.md),
[frontend profiling](selfhost-profiling.md) and [GC type ordering](selfhost-ordering.md)
and [direct syntax assembly](selfhost-syntax.md) for contracts, measurements and
the next boundary.


Concrete heap instances now cache their substituted composite shape and resolved
field/storage types. These immutable values are published only after successful
resolution. Array-storage and named-constructor queries reuse them too; source
opaque-access checks still run independently at each operation. Different
nominal declarations or normalized type arguments retain separate cache entries.
