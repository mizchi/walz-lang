# Component source and inferred adapters

Walz distinguishes a static library from an executable Core module or Component.
Standard `(module ...)`, `(component ...)`, `instantiate`, `canon lift`, and
`canon lower` retain their WebAssembly meanings. The extensions `@expose` and
`@bind` infer value adapters from checked implementation signatures.

## Static library and public API

```wat
(component
  (@world "example:calc@0.1.0" "app")
  (@lib
    (func $add (param $a i32) (param $b i32) (result i32)
      (@effect pure) (+ $a $b))
    (func $identity (@type.params $T) (param $value $T) (result $T)
      (@effect pure) $value))
  (@expose "add" $add)
  (@expose "identity" $identity<i64>))
```

`@lib` declares implementation code without a runtime entry, start function,
global/data initialization, or Core exports. Its functions may still allocate,
mutate data, or call explicitly declared imports. A file rooted at `@lib` uses
`@export` and selective `@use` just like a static source library. `walz check`
can check that file; build a module or Component entry to execute it.

`@expose "name" $function` makes a function a Component export. Type arguments
are closed by ordinary monomorphization first. Numeric types map to WIT scalars;
explicit core bytes/string/list/Option/Result and supported ABI-annotated
records/variants map to portable values. Arbitrary GC references, raw pointers,
`v128`, open generics, and `externref` have no implicit portable representation.
Effects, ownership, and any proof obligations are checked before adapter emission.

`@world` names the generated package and world. It does not assert a second,
unchecked signature. The build decodes the validated Component interface to
produce WIT, including public parameter names, async functions, future/stream
payloads, and nominal types. Explicit legacy scalar spellings such as i32 map
to s32; unsigned WIT contracts need the appropriate existing ABI types.

```sh
walz build examples/components/main.walz --wit
# target/main.component.wasm, target/main.component.wasm.json, target/world.wit
walz wit examples/components/main.walz
walz types examples/components/main.walz
walz component-wat examples/components/main.walz
```

The root selects the Component target automatically. `--wit` (also `--emit-wit`)
writes `world.wit` beside the output. `--wit-input FILE [--world NAME]` selects
an existing contract: for a module it builds the original native WIT world;
for a Component it checks its exact import/export ports and WIT types. Requested
WIT is validated before any output is written. Output aliases of inputs are rejected.
Generated WIT is parsed independently before it is returned. Contract checks
compare aggregate payloads, async status and nominal resource identity; merging
two WIT resolver graphs alone does not establish type equality. Local `use`
aliases may be renamed without changing the runtime ports.
Standard Components exporting Core modules remain accepted even if their API
cannot be expressed in WIT; asking for WIT then reports that limitation.

## Explicit connections and instantiation

```text
(component
  (@use "./counter.wasm" (component $Counter))
  (instance $counter (instantiate $Counter))
  (alias export $counter "next" (func $next))
  (@lib
    (import "counter" "next" (func $next (result i32)))
    (func $run (result i32) (@effect host) (call $next)))
  (@bind "counter" "next" $next)
  (@expose "run" $run))
```

`@bind "module" "member" $component-function` supplies the corresponding
Walz import. The actual Component function determines parameter names and async
status; final Component validation checks its full type against the checked
implementation. This also works for an imported or aliased Component function.
Calling it remains a host capability in the Walz effect system; a binding cannot
make a call pure or override the compiler's fixed host contracts. Duplicate ports
and incompatible signatures are errors. All live imports need explicit bindings
or their usual independently checked host policy.

Selecting `(component $Counter)` loads a constructor, from a relative `.walz`
Component source or `.wasm` Component binary. It does not instantiate it. Two
explicit instantiations have independent defined memories and globals; multiple
aliases of one instance share state. Standard instance declarations and their
initialization order survive compilation, even if no exported function uses them.
Binary dependency custom sections are preserved. Existing function-selecting
`@use "./library.wasm" (func ...)` retains its more restricted static-linking rules.

The complete [example](../examples/components/main.walz) combines a generic
static library, two counter instances, an explicit binding, bytes, and native
Future/Stream operations.

## Generic Component constructors

```wat
(component
  (component $Echo (@type.params $T) (@where (shared $T) (nonunit $T))
    (@lib (func $echo (param $value $T) (result $T) (@effect pure) $value))
    (@expose "echo" $echo))
  (instance $a (instantiate $Echo<i32>))
  (instance $b (instantiate (@inst $Echo i32)))
  (instance $wide (instantiate $Echo<i64>))
  (export "a" (instance $a))
  (export "b" (instance $b))
  (export "wide" (instance $wide)))
```

`@type.params` follows a Component constructor's name. An optional `@where`
immediately follows it and uses the existing generic constraints. Arguments are
explicit, compile-time value types. The ordinary Walz checker resolves them in
the caller's static type scope and checks constraints before specializing the
body. Import standard types explicitly, including aliases, inside a caller
`@lib` or a type-selecting `@use`. Type-only libraries need no `@expose` and emit
no runtime implementation. Numeric types (including internal `v128`), unit,
bytes/string and closed List/Option/Result/pair/box/Future/Stream/task combinations
can be arguments. A private
named alias to one of these values is normalized before specialization.

The same constructor and normalized arguments share one definition in a scope;
`instantiate` still creates independent state and fresh defined resources.
Aliases of one instance still share that instance. Type substitution preserves
term names, strings, opaque annotations and inner generic binders. Nested
constructors may capture closed outer type parameters. The concrete body passes
the existing type, bound, effect, ownership, proof and Wasm validation gates.
Owned endpoints still require explicit `@move`; a `shared` constraint rejects
them. Use a consuming constructor when its implementation transfers ownership.
Do not redeclare a constructor parameter as a type or type import in the same
library scope; an inner declaration's own `@type.params` can shadow it.

`@use "./generic-echo.walz" (component $Echo)` keeps source parameters until an
application requests a concrete definition. A root generic source is a template,
not an executable build entry. Binary `.wasm` constructors are already concrete
and reject type arguments. An explicit constructor export can specialize a
source template without instantiating it; its native Component export may have
no WIT projection. WIT generated for instantiated functions contains concrete
types, never open type parameters.

Closed visible struct, array, variant/enum and scalar-newtype definitions can
also be arguments, including checked generic type applications. Write a heap
value as `(ref $Item)` (or a value alias); `$T` in a heap operand selects that
argument's heap declaration. The compiler copies the reachable type definitions
into each implementation's private static scope, preserving recursion, packed
storage, nullability, mutability, field/case names and ABI annotations. It copies
no object, memory, global or resource. Source nominal identity participates in
specialization caching: aliases share a definition, distinct source declarations
and phantom generic arguments remain distinct even when layouts match.

Opaque representations, closures and resource/borrow identities cannot be copied
as constructor arguments. Opaque layouts report `E_OPAQUE_REPRESENTATION`; use
the owning library's functions or an explicit runtime interface instead.
Ordinary type/effect/ownership/proof checks still apply to every concrete body.
Public values still need portable WIT types:
`v128`, boxes and task handles may be used internally but cannot be exposed
implicitly. Pass resource identity via
typed runtime instance imports and `@bind`, and keep shared linear memory an
explicit runtime connection. Named public values receive native type ports;
exported instance ascriptions give each interface its own value naming aliases,
checked by Component subtyping. Imported resources keep their actual defining
ports. No conversion, destructor or runtime call is introduced by an ascription.
Export an instantiated interface containing its named values and functions;
a constructor with direct named type exports can be valid native Wasm without
a standalone WIT world in the pinned decoder. `--wit` still requires a supported
projection of the final entry. Generic constructors do not add higher-kinded
types, type inference for constructor arguments or runtime type parameters.
In a scope containing generic constructors, use named Component references and
named Component outer aliases. Numeric Component references are diagnosed to
prevent generated definitions from silently changing their indices. Native
WAT without constructor generics keeps numeric references unchanged; Core
instruction indices are unaffected.

Expansion is bounded across nested compilation: 1,024 specializations, 64
Component compilation levels, one million generated syntax nodes and 16 MiB of
argument-checking input. Errors retain the definition and application locations.
See [the runnable entry](../examples/components/generics.walz) and its
[generic source dependency](../examples/components/generic-echo.walz) for shared
values, nested carriers, owned endpoint transfer and asynchronous consumption.

## Connected type queries

`@component.type` reads the actual, validated graph selected by an explicit
`@bind`. It introduces no import or capability. Use it in value-type positions,
including transparent aliases, function signatures, typed lets and generic
function arguments:

```text
(@type.alias $Input (@component.type "host" (param "echo" 0)))
(@type.alias $Output (@component.type "host" (result "echo")))
(@type.alias $Item (@component.type "host" (type "item")))
(@type.alias $Element (@component.type "host" (result "echo") option.element list.element))
(@type.alias $Field (@component.type "host" (type "item") (field "value")))
```

The namespace is the local connection name, not a guessed WIT package or file.
`param` takes a zero-based decimal index or the WIT parameter name as a string.
`result` selects the result; `type` selects a named export of an instance binding.
Individual function bindings support parameter/result queries, without granting
access to their whole defining interface. Projections can chain
`option.element`, `list.element`, `result.ok`, `result.err`, `tuple.first`,
`tuple.second`, `future.element`, `stream.element`, `(field "name")`,
`(case "name")` for variants and `(item index)` for arbitrary tuples.
A missing result or case payload is unit: it can be a constructed-value payload;
ordinary unit functions still omit their result declaration.

Unsigned/narrow scalars keep their checked ABI annotations. Named records,
variants, enums, flags and larger tuples keep shared payload restrictions and
are attached to the selected instance's type exports. A named type used by the
public API may require a type-only instance port even when no host function is
called. Incompatible source representations do not unify by spelling. Helper
types are private to the static module; share a source type explicitly instead
of independently creating two nominal storage types for one WIT type. A value
alias to a record is not a new nominal heap declaration; use the ordinary alias
and field-pattern rules.

Resource queries require one explicit local `@resource` declaration for the
actual defining resource. They reuse that owner/borrow type and never synthesize
a destructor import. Missing/ambiguous anchors, wrong selectors and non-type
uses report `E_COMPONENT_TYPE`. Core-only sources report
`E_COMPONENT_TYPE_CONTEXT`; opaque WAT annotations and strings remain data.
Normal type, effect, ownership, ABI and proof checks still run after projection.

Implementation queries resolve before the ordinary checker. Constructor
arguments can also query a connection backed by native declarations **preceding
that constructor application**, including already specialized instances:

```text
(instance $first (instantiate $Echo<i64>))
(@bind "first" (instance $first))
(instance $second (instantiate $Echo<(@component.type "first" (result "echo"))>))
```

Binding declarations may appear later; their target graph must already be
available. Only queries reachable from the requested arguments participate in
that stage. An unused alias may refer to a later connection and is still checked
with the final implementation. The compiler validates the preceding graph
without running start functions, host calls or metadata. Forward/cyclic
constructor dependencies report `E_COMPONENT_TYPE_PHASE` at the application.
Real types, never discovery witnesses, enter constraint checking and snapshots.
Probe-only witnesses for unused aliases never reach implementation IR or output. Limits are
1,024 queries, 64 projection steps/type depth, a 64 KiB expanded type-shape budget
and 16 MiB of generated type syntax per Component implementation.

See [the query example](../examples/components/type-queries.walz) and
[typed constructors](../examples/components/typed-constructors.walz), which
combines record queries, staged producers and owned Future/Stream transfer.
Nominal snapshots are additionally bounded to 64 levels, 1,024 reachable nominal
definitions, 65,536 type visits and 16 MiB; native export ascriptions are bounded
to 1,024 instance exports and 16 MiB.

## Resource interfaces and ownership

Use the instance form when a connection includes resource types:

```text
(@bind "test:resources/host" (instance $files))
```

The selected instance supplies its exported type graph and function contracts.
Walz still declares each import explicitly. A source resource declaration such as
`(type $File (@resource "test:resources/host/file" $drop))` is checked against the
actual defining resource, including any aliases/projections of the instance.
A matching filename, representation integer, or function shape cannot establish
resource identity. The compiler uses validator type IDs for import aliases and
keeps a single decoded graph for all connections.

The source destructor import has the usual name `[resource-drop]file` and an
`i32 -> unit` implementation signature. Its Close contract is inferred only for
the declared resource destructor, and the adapter supplies `canon resource.drop`;
the host does not implement a function of that name. Unrelated numeric factories
are not interpreted as own handles. Namespaces cannot combine an instance binding
with function bindings or override fixed host contracts. Function bindings in
other namespaces can coexist with resource bindings.

`@expose` infers own/borrow and supported Option/Result/pair carriers from the
checked source. Owners still require `@move`; a borrow neither closes nor becomes
an owner, and `@using` closes its owner on normal return and handled exceptions.
Generic owned functions are monomorphized before that inference. Public signatures
refer to the imported resource's external type aliases, not private adapter types.
WIT output therefore preserves the original defining resource. Each generated
WIT scope gets explicit `use` declarations for referenced interface types, with
collision-free local aliases. An external WIT file or hand-written
binding-policy JSON is not required for these explicit
connections; the existing `--wit-input` backend remains available.

[The resource example](../examples/components/resources.walz) imports a native
interface and exposes make/read/echo/close, optional owners and handled cleanup.
Supply `options.imports['test:resources/host']` to the JavaScript builder; its File
class and functions follow jco's native resource convention, including
`Symbol.dispose`. Node/Chromium tests check object identity, borrow lifetime,
one destructor call and ABI-buffer retirement. Node also checks collection of a
closed host object with a WeakRef.

Private Component providers can also keep resources entirely behind a scalar
public API; see the [scoped-provider fixture](../tests/component-fixtures/composition-private-resources.walz).
Their constructor identity and destructor state remain per instance. A locally
defined provider uses a qualified WIT namespace for its resource instance binding;
an alias of an imported resource can use a local binding namespace because its
defining interface is already known. Raw Core
providers must distinguish imported borrowed handles from borrows of a resource
they define: the latter receive the representation directly, as specified by the
[Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md#lowering-values).
Walz-generated imported-resource wrappers retire their borrowed input handles.
A resource appearing in a public signature
must have an externally nameable definition. Composition does not implicitly
publish all of a private provider's resource interface or grant its functions.

Lists, ABI records/variants and Future/Stream payloads remain restricted to shared
values. Native trap/cancellation behavior retains the instance/store lifetime
rules below. Some valid native instance/type reexports have no supported WIT
projection; they remain valid native output and WIT requests return a diagnostic
instead of entering the pinned decoder's duplicate-alias assertion.

## Memory and asynchronous values

A Core memory index is local to one Core instance. Distinct instantiations get
fresh memory identities; explicit imported aliases retain the original identity.
The build manifest's `memoryGraph` records observed uses and canonical adapter
contexts. Opaque host definitions/unsupported alias paths are reported as
unresolved, rather than assumed independent. This is an inspection graph, not a
proof of race freedom or of arbitrary host behavior.

Each generated ABI adapter has a named memory. Generated loads/stores refer to
that memory; source data-memory instructions keep their own checked references.
Canonical operations select their memory/reallocator explicitly. Cross-component
strings and lists undergo canonical conversion; their GC identity and pointers
are not shared. Raw standard Component syntax can wire multiple memories
explicitly, while inferred public APIs use portable values.

Native future/stream producers retain input copies until a write is acknowledged,
handle partial stream writes, and transfer endpoint ownership through the
Canonical ABI. Returning a readable endpoint does not mean its producer has
finished. A cancellation request alone does not authorize freeing its buffers.
These sources use the native Component path, without private Walz delivery imports.

For JavaScript, `runtime/build.mjs` returns `{api, exports}` containing the native
exports and accepts explicit `options.imports`, `getCoreModule`, and
`instantiateCore`. Its pinned jco JSPI adapter may return a Promise for a call
that crosses Component instances; callers can `await` it. Future/Stream values
use Promises and async iterables; numeric stream chunks can be typed arrays.
SDK child tasks, SDK cancellation/deadline APIs, and persistent runtime wrappers
are not automatically installed for arbitrary native Components.
The build applies a narrow correction to pinned jco's async host-result glue:
unused duplicate result allocations are retired after serialization, and host
rejections propagate to the JSPI caller. It preserves guarded realloc calls and
does not free a pending transfer or install SDK scheduling/lifetime policy.

Native failures follow the VM's instance/store lifetime. A failed async call or
trap can poison an instance; dispose of that instance/store and its resources.
Rejecting a host Promise is a native failure, including when it supplies a
transferred readable endpoint; use Result payloads for recoverable application
errors instead of relying on a rejected JS Promise to survive native forwarding.
Success-path zero live-buffer checks do not promise that a trapped instance can
be reused or that its remaining buffers are individually reclaimed. Existing
Core+JSPI/SDK cleanup contracts are unchanged. See [native worlds](native-worlds.md).

## Core FFI metadata and current scope

An external Core module can describe a JS-facing signature using standard
`@custom`. The reserved `walz:ffi-v1` section contains strict UTF-8 JSON:

```json
{"version":1,"functions":[{"export":"identity","params":["externref"],"results":["externref"]}]}
```

Attach it inside the Core module. Every named export and its parameter/result
types must match the actual Wasm binary. Supported descriptors are i32, i64,
f32, f64, v128, funcref, and externref. Unknown fields, duplicate sections,
duplicate function descriptions, and mismatches are rejected. Other custom
sections remain opaque. This metadata grants no purity, ownership, or host
authority; externref stays a Core/JS value. Portable opaque handles use resources.
`walz:world-v1` is reserved for the compiler's package/world metadata.

Inference covers portable shared values, future/stream endpoints and imported
resource own/borrow through explicit instance bindings. Function and source
Component constructor specialization are supported with the boundaries above.
Arbitrary host memory proofs, runtime type parameters, copying opaque/resource
representations, cyclic constructor inference and native SDK task scheduling
remain outside this composition layer.

Run `just test-composition`, `just test-composition-e2e`, and `just test-runtime-cli`.
The tests check actual Wasm validation and execution, type/effect failures,
constructor identity, memory aliases, Core+JSPI/data-memory separation, async
bindings, Unicode/bytes/stream transfer, normal buffer retirement, and native
failure isolation. The [Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md)
and [concurrency model](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md)
remain the runtime semantics.
