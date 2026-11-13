# Bootstrap modular frontend

The compiler written in Walz implements static modules, fixed-form lowering,
GC effect bounds, a minimal core library, monomorphization and declarative user
macros toward self-hosting. The Rust compiler still builds the compiler and its
Core/JSPI or Component adapter. Compiling the compiler's own complete
sources and comparing stage 1/2 artifacts remain future work.

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
      → specialization and typed core expansion
      → lexical/control lowering and GC recursion-group ordering
      → folded source + origin map
      → reparse → independent type/stack/effect validation
      → opaque checked module → binary emitter
```

The frontend's public immutable trees and expanded text are untrusted data.
Only the independent validator creates the opaque module accepted by the emitter.
Every emitted instruction is checked, including unreachable code. Fixed-form
expansion is not a substitute for type, effect, ownership or proof checking.
Unsupported contracts are rejected rather than silently removed.

Ordinary folded WAT submitted through `compile`/`check` keeps the original
lower-level path and its canonical-output contract. Extended source and all
project API calls use this frontend. Their output is checked by independent
validators and execution; it is not promised to be byte-identical to Rust's
high-level lowering.

## 1. Static modules

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
  Arbitrary package namespaces, wildcard imports and type/function aliases or
  source reexports are not implemented. Macros support selected imports, aliases
  and explicit reexports. Cycles and duplicate file identities are errors.
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
checked non-null reads.

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

Option/Result/Tuple/List payloads are currently shared and nonunit. Unit-valued
container payloads, owned resources, futures and streams are not ported.
Match checks variant/payload arities and exhaustiveness; only the selected
branch executes, and its scrutinee is evaluated once. Arbitrary user enums and
object/record patterns remain future work.

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

`shared` and `nonunit` are the supported constraints. `shared` permits unit;
`nonunit` excludes it. Unknown, open, malformed or wrong-arity arguments are
errors even when a parameter is unused. Other typeclasses/callable constraints
are rejected. Generic source API exports are permitted; a native Wasm export
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
GC types at 128, cached specializations (including aliases) at 256, work at
262,144 steps, nested type/expression/lowering traversal at 128, generic-angle
parsing at 64 and rendered folded source at 1 MiB. The independent parser/checker
also impose their [existing limits](selfhost.md#resource-limits-and-lifetime).
Macros additionally have limits of 64 rules per definition, 512 invocations,
64 nested invocations and 100,000 charged substitution/hygiene visits. Existing
128-level syntax/traversal and 1 MiB rendering limits still apply. Query probes
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

Tests cover both JS adapters and Chromium, the native project example, malformed
forms, lazy branches, source identity/ranges, opacity after specialization,
explicit recursive groups, generic caching/growth limits, effect unions,
snapshots, macro hygiene/repetition/delegation, type-query errors and ABI cleanup
after success/errors and forced GC. Emitted modules
also pass independent Wasm validation. ABI counters do not prove reclamation of
every guest GC allocation.

Remaining work includes ownership/proof and ABI
lowering, closures/callable constraints, the rest of core, floating point/SIMD,
multi-value control, memories and imports, and complete recursive type
equivalence. WIT/component imports and async runtimes are not output features of
this backend. Stage 1/2 reproducibility is still required for full self-hosting.
Historical benchmark and browser-size captures predate GC, this
frontend and user macros; preserve them and create a new capture before comparing performance.
