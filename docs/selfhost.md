# Self-hosting compiler parts

Walz has a source parser, a type/stack checker and a binary backend written in
Walz. The backend supports integers and a bounded Wasm GC subset, including
recursive structs/arrays and references. These are bootstrap components. The
Rust compiler remains the main implementation. A [modular frontend](selfhost-frontend.md)
adds static modules, fixed forms, effect bounds, minimal core, monomorphization
and declarative expression macros.
This subset compiles its own sources into Core Wasm and passes a
[two-stage fixed-point gate](selfhost-stages.md). Full ownership/proof/ABI
lowering remains part of the broader Rust feature-parity work.

```text
Rust compiler ──build──> compiler.wasm
source bytes ──Walz parser──> immutable syntax
             ──GC type collection, names and inference──> untrusted immutable IR
             ──independent type/operand/control checks──> opaque checked module
             ──Walz binary encoder──> output.wasm
```

The running `compiler.wasm` uses no compiler host import, Rust parser, `wat`
process or JS code generator. Node loads the compiler and handles file I/O.
The output contains Core Wasm functions and, when used, Wasm GC types/instructions.
The compiler itself uses Wasm GC.
`examples/selfhost/portable.walz` exports `compile`, `check`, `compile-project`,
`check-project` and `imports`; see the [project API](selfhost-frontend.md#api-and-validation-boundary).
AST inspection,
raw-IR validation, byte-builder misuse and LEB test endpoints live separately in
`tests/component-fixtures/selfhost.walz`.

## Try it

With the repository's Rust toolchain and Node.js 24+ installed:

```sh
just build-selfhost
just selfhost-check examples/selfhost/sum.walz
# Checked examples/selfhost/sum.walz (2 functions)
just selfhost-compile examples/selfhost/sum.walz target/selfhost-sum.wasm
node --input-type=module -e 'import {readFile} from "node:fs/promises"; const {instance} = await WebAssembly.instantiate(await readFile("target/selfhost-sum.wasm")); console.log(instance.exports.main());'
# 5050n
just selfhost-compile examples/selfhost/gc.walz target/selfhost-gc.wasm
node --input-type=module -e 'import {readFile} from "node:fs/promises"; const {instance} = await WebAssembly.instantiate(await readFile("target/selfhost-gc.wasm")); console.log(instance.exports.main());'
# 42n
```

`build-selfhost` uses the existing Rust compiler once to create the Core/JSPI
adapter in `target/selfhost/`. Rebuild it after changing compiler sources.
`selfhost-check` resolves and validates source without emitting bytes;
`selfhost-compile` also emits a binary. Malformed/unsupported input returns a
diagnostic and the file helper exits with status 1. It writes output only on
successful compilation and independently validates the bytes first.

## Modules and contracts

| Module | Responsibility |
| --- | --- |
| `walz/parser/parser.walz` | UTF-8 input, parentheses, atoms, strings, comments, source ranges and immutable postorder syntax |
| `walz/parser/string.walz` | WAT byte escapes and Unicode scalar escapes |
| `walz/parser/number.walz` | Checked decimal/hex i32/i64 constants, including positive unsigned bit patterns |
| `walz/parser/diagnostic.walz` | Shared `{code, start, end}` record; half-open byte ranges |
| `walz/frontend/` | Static modules, source origins, bounded specialization, minimal core and fixed-form lowering; see the [frontend guide](selfhost-frontend.md) |
| `walz/checker/ir.walz` | Untrusted, immutable GC type/field/instruction/function/export records |
| `walz/checker/operators.walz` | Closed scalar opcode, type, arity and alias tables |
| `walz/checker/gc-operators.walz` | Closed GC source opcode table |
| `walz/checker/syntax.walz` | Shared syntax access and ASCII identifier checks |
| `walz/checker/type-env.walz` | Source GC names, fields, recursive groups and reference resolution |
| `walz/checker/gc-types.walz` | Raw type descriptors, field variance and reference subtyping |
| `walz/checker/checker.walz` | Declarations, names, lexical scopes, inference, lowering and the opaque checked-module API |
| `walz/checker/stack.walz` | Independent GC/type/operand/control-stack validation of raw IR |
| `walz/checker/gc-effects.walz` | Conservative mutable GC effect detection and direct-call propagation |
| `walz/codegen/encoding.walz` | Canonical unsigned 32-bit and signed 32/64-bit LEB128 |
| `walz/codegen/types.walz` | GC recursive groups, subtype definitions and value/storage type encoding |
| `walz/codegen/emit.walz` | Wasm header, type/function/export/code sections and instruction encoding |
| `walz/codegen/compiler.walz` | Parse → check → optional emit Result pipeline |
| `walz/core/bytes-builder.walz` | Bounded private mutable GC byte storage; copied immutable publication |

Select parser, checker and compiler APIs through relative `@use` paths. The byte
builder is also selectable from `walz:core`. These libraries add no language
grammar or ownership rule.

```wat
(@use "../walz/codegen/compiler.walz"
  (func $compiler.compile) (func $compiler.check))
;; compile(bytes) -> $result<$bytes, (ref $diagnostic)>
;; check(bytes)   -> $result<i32, (ref $diagnostic)>  ;; function count
;; Both have @effect reads writes allocates.
```

The checker can be used independently of binary emission:

- `$checker.check(syntax)` resolves and lowers an opaque parser snapshot, then
  validates the resulting IR.
- `$checker.validate(functions, exports)` validates raw immutable IR directly.
- `$checker.validate-gc(types, functions, exports)` also validates GC descriptors.
- All return `$result<(ref $module-ir), (ref $diagnostic)>`.
- `$codegen.emit(module)` accepts only this opaque checked module.

Only the checker constructs `$module-ir`. Its getters expose immutable lists and
records; callers cannot mutate a validated function or instruction afterward.
Raw IR records are public data, not validation certificates. Function signatures
are checked before bodies, so forward and recursive calls receive the same checks.

IR types use Wasm bytes `127` for i32, `126` for i64 and `64` for an empty result.
Reference tokens are `512 + 2 * heap + nullable`, where nullable is 0 or 1.
Heap is a declared GC type index or one of `-18` any, `-19` eq, `-20` i31,
`-21` struct, `-22` array and `-23` none. Storage types also allow `120` i8 and
`119` i16, which unpack to i32 values. Token 0 is internal stack unknown,
never a declared value type.

Each `$field-ir` has `{storage, mutable}`. Each `$gc-type-ir` has
`{kind, fields, parent, finality, start, end, explicit}`: kind is struct=0 or
array=1; parent is an earlier type index or -1; finality is 0 or 1. `start/end`
are the recursion group's type indices (end exclusive), not source positions.
Explicit is 1 for a `(rec ...)` group or 0 for an implicit singleton. All members
must agree on group boundaries and explicitness. Fields may reference earlier
types or members of their own group. An array has exactly one field descriptor.

Instructions carry `{op, immediate: i64, start, end}`. i32 constants use canonical
signed-32 immediates; index immediates are unsigned-32; unused immediates must be
zero. GC opcodes use `256 + GC subopcode`, emitted with the `0xfb` prefix.
For struct field access, array.new_fixed and array.copy, the low 32 immediate
bits contain the type/destination type index and the high 32 bits contain the
field/count/source type index. Single-index GC instructions require zero high
bits. `ref.null`, ref.test and ref.cast carry a signed heap code; typed select
carries a value-type token. A function's body excludes its final implicit `end`,
which the emitter adds.
The checker verifies opcodes, immediate ranges, locals/calls/labels, operand
types, stack heights and structured-control nesting. This is a validator for the
supported IR, not a reader or validator for arbitrary Wasm binaries.

The operand stack follows Wasm's unreachable-code rules. At an unreachable
frame's stack floor, a pop may obtain an unknown value. Concrete values pushed
after that point still have types: an i32 cannot satisfy an i64 operand merely
because execution is unreachable. `br_if` retains the declared label value when
the branch is not taken. A loop label takes no arguments in this subset; block,
if and function labels use their declared result.

## Source parser

`$parser.parse(bytes)` returns Result of an opaque syntax snapshot or a diagnostic.

For generated code, `$node.new` creates untrusted immutable node descriptions;
`$syntax.assemble` validates postorder references, token shape, depth and finite
storage budgets before creating opaque syntax with virtual node positions.
`$node.indexed` is an untrusted position hint: a fully validated indexed arena
may be reused, and other descriptions are normalized. No structural check is
skipped by that fast path.
It does not create checked IR. The frontend retains a separate origin map and
runs the same independent checker. See [direct syntax assembly](selfhost-syntax.md).

`$parser.parse` accepts zero or more S-expressions; requiring one `module` is the
checker's job. It checks source UTF-8, retains the original immutable bytes, skips `;;` line
comments and nested `(; ... ;)` comments, and decodes WAT strings:
`\hh`, `\n`, `\r`, `\t`, `\'`, `\"`, `\\`, and `\u{...}`.
Byte escapes may produce non-UTF-8 bytes; export names are checked separately.
Raw controls and invalid Unicode scalars are rejected.

`$syntax.nodes` returns a postorder list; `$syntax.roots` returns its root indices.
Children precede their parent. `$node.kind` is atom=0, string=1, list=2,
quoted identifier=3, Walz curly field group=6. Parsed start/end ranges include delimiters and count **UTF-8 bytes**, not JS
UTF-16 units. Atom values retain their spelling; string/quoted-ID values contain
decoded bytes. `$node.children` is an immutable list of indices. Comments remain
in `$syntax.source`, not as AST nodes.

This is a structural S-expression reader, not the full Rust Walz reader. Unknown
atom spellings are preserved. The separate frontend normalizes generic application
sugar and resolves multiple source files; the parser itself does not elaborate
them. Typed named construction uses curly field groups in the frontend; untyped
object literals remain unsupported. Declarative expansion/hygiene runs in the
separate frontend. Quoted
identifiers are parsed but currently rejected by the checker.

## Lower-level compilation subset

This section describes the original folded-WAT checker path. See the
[modular frontend](selfhost-frontend.md) for the additional high-level forms.

The [sum example](../examples/selfhost/sum.walz) uses mutable i64 locals and a
labelled loop. A smaller Walz-style example is:

```wat
(module
  (func $add (param $a i64) (param $b i64) (result i64)
    (@effect pure) (+ $a $b))
  (func $main (export "main") (result i64)
    (@let $x (type i64) 40
      (call $add $x 2))))
```

- One `module`, optionally named, with GC type declarations, functions and native WAT exports.
- Individual parameters and mutable locals: `(param $name i64)`, `(param i32)`,
  `(local $name i64)` or `(local i32)`. Parameters precede results and locals;
  all declarations precede the body. Grouped declarations are not supported.
- An omitted result or `(result)` means unit. Otherwise one i32/i64/reference result.
  Bodies contain zero or more folded expressions/statements.
- Bare numbers use the expected i32/i64 type; explicit `i32.const`/`i64.const`
  fix it. Decimal/`0x` literals allow signs and digit separators. Positive
  constants may express all u32/u64 bit patterns; negative magnitudes are bounded
  by the corresponding signed minimum.
- Named/indexed `local.get/set/tee` and direct calls, including forward calls
  and recursion. Numeric locals start at zero and nullable reference locals at
  null. Non-null declared locals are rejected until initialization analysis is
  implemented. Ordinary ASCII WAT identifiers work.
- For both integer widths: `eqz`, `eq/ne`, signed/unsigned `lt/le/gt/ge`,
  `add/sub/mul`, signed/unsigned `div/rem`, `and/or/xor`, `shl/shr_s/shr_u`.
  Conversions: `i32.wrap_i64`, `i64.extend_i32_s`, `i64.extend_i32_u`.
- Fixed aliases `+ - * / = != < <= > >= & | ^ << >> >>>`; division and ordered
  aliases are signed. Operands supply type hints, or arithmetic uses its expected
  result type. Comparisons with only untyped literals are ambiguous: write
  `(< (i64.const 1) 2)` or `i64.lt_s`. There are no implicit numeric conversions.
- `block`, `loop` and `if`, with optional labels and one result annotation.
  `if` takes `(then ...)` and optional `(else ...)`; a value-producing if needs
  both. Bodies can contain multiple expressions. `br`, `br_if` and `return`
  check label/result types; `nop`, `unreachable`, `drop` and `select` work too.
- Single-binding `(@let $x (type i64) initial body)` or inferred
  `(@let $x initial body)`. Inference needs a scalar type hint in the initializer.
  Reference initializers can also supply a type hint. The binding is immutable
  and lexical; shadowing works. Non-null reference lets use nullable physical
  local storage and restore non-nullness on reads dominated by initialization.
  Numeric local indices can address only declared parameters/locals, not hidden
  let storage.
- Inline `(export "name")` and standalone `(export "name" (func target))`,
  with decoded UTF-8 names, embedded NUL, Unicode and duplicate checks.
- Optional `(@effect pure)` or `(@effect reads writes allocates)` bounds are
  checked conservatively. Immutable GC allocation
  and reads are pure; allocation of mutable GC storage, mutable-field reads and
  writes are not. Direct calls propagate these effects to a fixed point,
  including calls through unannotated functions. Unreachable instructions are
  included. Mutating a function's own locals does not expose an external effect.
  Only these GC effect names are supported; full effect/ownership/proof checking
  is not ported.

### GC subset

Use ordinary WAT declarations and folded instructions:

```wat
(module
  (type $node (struct (field $value i64) (field $next (ref null $node))))
  (func (export "main") (result i64)
    (struct.get $node $value
      (struct.new $node (i64.const 42) (ref.null $node)))))
```

The [GC list example](../examples/selfhost/gc.walz) builds and traverses three
recursive nodes, returning 42. The supported GC forms are:

- `struct` fields and `array` elements with i32/i64, i8/i16 or internal references;
  optional field/type names and explicit `(mut storage)`.
- Implicit self recursion, explicit `(rec (type ...) ...)` mutual recursion,
  `(sub ...)`, optional `final` and one earlier supertype. Struct width extension,
  immutable-field covariance and mutable-field invariance are checked.
- `(ref heap)` and `(ref null heap)`, with named/indexed declared types or
  abstract `any`, `eq`, `i31`, `struct`, `array` and `none` heaps.
- `struct.new`, `struct.new_default`, `struct.get`, `struct.get_s/u`, `struct.set`;
  `array.new`, `array.new_default`, `array.new_fixed`, `array.get`, `array.get_s/u`,
  `array.set`, `array.len`, `array.fill` and `array.copy`.
- `ref.null`, `ref.is_null`, `ref.eq`, `ref.as_non_null`, `ref.i31`, `i31.get_s/u`,
  `ref.test (ref [null] heap)` and `ref.cast (ref [null] heap)`.
- Reference parameters/results and inline block/if results, including typed
  `(select (result (ref null $type)) first second condition)`.

Reference matching follows declared type indices and their parent chains. It
does **not** yet implement structural equivalence between separately declared
recursive groups. Some valid Wasm programs are conservatively rejected. Distinct
indices must not be used to optimize reference equality or casts. Abstract
subtyping follows Wasm's internal heap hierarchy: `any` is not a subtype of `eq`.
Packed loads require the signed/unsigned getter; writes require mutable storage;
default constructors require defaultable fields. These checks also apply to raw IR.

Branches stay lazy. Arithmetic wraps or traps according to Wasm; no constant
folding or reassociation is performed. Every instruction is validated, including
unreachable code. Native `export` is distinct from Walz `@export`.

Unsupported constructs return errors: floats, SIMD, multiple results/block
parameters, function/external/shared references, descriptor types, abbreviated
reference spellings such as `anyref`, memories, imports and flat
stack-style WAT. `@use`, multi-binding let and other bounded high-level forms go
through the [modular frontend](selfhost-frontend.md). The existing Rust compiler
remains the implementation for features outside that subset. For example, a valid stack-style
operandless `(drop)` is not accepted as folded source, although the raw-IR
validator correctly checks its operand-stack behavior.

## Resource limits and lifetime

Limits: 1 MiB source, 65,536 syntax nodes, 128 nested lists, 1,024 GC types,
256 fields per type and 8,192 fields in total, 1,024 functions,
1,024 exports, 256 parameters and 1,024 locals per function (including let
storage), 262,144 total IR instructions, 128 explicit control frames and 1 MiB
of export-name bytes. Raw IR ranges must satisfy `0 <= start <= end <= 1 MiB`.
The emitter's private output builders are capped at 16 MiB; the IR budgets bound
valid output below that capacity.

The general byte builder accepts capacities from 0 to 16 MiB. Byte/range/capacity
misuse traps; compiler input errors use Result. `finish` copies and leaves the
builder usable, so older snapshots remain unchanged after append or growth.
Syntax intentionally retains source bytes; checked IR retains numeric ranges
and decoded export names, not the syntax arena. Drop references to allow Wasm GC
to collect these values. Output ABI buffers are released after lifting bytes.

Traps poison Component-style adapter instances; misuse tests discard them after
verifying cleanup. Result errors keep the compiler instance usable. Compilation
uses no external host resources or suspension points, and currently has no
cooperative cancellation point.

## Diagnostics

| Code | Meaning |
| ---: | --- |
| 1 | Invalid source UTF-8 |
| 2 | Unexpected token byte |
| 3 | Missing/mismatched parenthesis or expression |
| 4 | Unterminated block comment |
| 5 | Invalid/unterminated string or escape |
| 7 / 8 / 9 | Source / syntax-node / nesting limit |
| 100 | Expected exactly one module |
| 101 | Unsupported form or malformed structural shape |
| 102 / 103 | Duplicate / unknown function |
| 104 | Invalid signature or duplicate parameter/local name |
| 105 | Invalid/out-of-range integer constant |
| 106 | Wrong operand or call arity |
| 107 / 108 | Duplicate / invalid UTF-8 export name |
| 109 | Unknown/out-of-range local |
| 110 / 111 | Type or stack-height mismatch / operand-stack underflow |
| 112 | Unknown/out-of-range branch label |
| 113 | Assignment to an immutable let binding |
| 114 | Invalid IR opcode or immediate |
| 115 | Invalid control structure |
| 116 | Ambiguous scalar type; add an annotation |
| 117 | Invalid IR source range |
| 118 | Invalid or unsupported value/GC type, recursive group or subtype |
| 119 | Unknown/incompatible GC type, field or index |
| 120 | Write to immutable GC storage |
| 121 | Non-defaultable GC constructor field |
| 122 | Packed/unpacked getter mismatch |
| 123 | Mutable GC effects exceed the declared effect bound |
| 150 | Function, export, parameter, local, instruction, control or frontend work limit |
| 200 | Missing/invalid module path or file identity |
| 201 | Unknown, private or duplicate source symbol |
| 202 | Access to a foreign opaque representation |
| 203 | Static module cycle |
| 205 | Malformed loop or invalid recur placement/arity |
| 206 | Malformed generic declaration/application or open native export |
| 207 | Unsupported or unsatisfied type constraint |
| 208 | Invalid or incomplete core match |
| 209 | Invalid UTF-8 string literal |
| 210 | Malformed generic application spelling |
| 211 | Invalid macro definition or clause |
| 212 / 213 | Unknown macro / no matching rule or type guard |
| 214 | Invalid macro template or repetition lengths |
| 215 | Unbound definition-site template name |
| 216 | Invalid macro binding scope or delegated interface |
| 217 | Macro expansion budget |
| 218 | Invalid type query/projection or missing expected type |
| 219 | Retired `@bind`; use `@connect` for Component connections |
| 220 | Binary function/type selection or retired implicit Component descriptor |
| 221 | Component composition/constructor requires the main compiler |
| 222 | Static `@lib` has no executable entry |
| 223 | Core ABI export or initialization in a static `@lib` |
| 224 | Malformed named construction or unsupported curly field group |
| 225 | Malformed, unsupported, recursive or nonportable source ABI view |
| 226 | Source ABI view exceeds layout, flat-slot, shape, depth or work limits |

## Verification and next steps

```sh
just test-selfhost
just test-selfhost-e2e
just test-selfhost-bench
```

Checks cover native Wasmtime, Core/Component adapters, Chromium, malformed
UTF-8/escapes, Unicode ranges, limits, canonical LEB128, wide indices, calls,
unit/i32/i64/reference functions, mutable and lexical locals, typed control, runtime traps
and snapshot isolation. Generated i32 expressions are compared against JS
wrapping arithmetic and the Rust compiler. Standard-WAT inputs use the pinned
`wat`/`wasmparser` crates as an independent reference, including empty functions
and multi-statement bodies. About 600 raw IR cases per adapter are compared with
independently encoded modules and `wasmparser`; both accepted and rejected cases
are included. GC adds 22 execution programs per adapter, plus a recursive-list
example, and 405 raw cases per adapter: 256 heap/nullability pairs, 128 generated
constructor/getter cases and 21 hand-written valid/invalid cases. Core and
Component also run the GC corpus in Chromium. The [frontend corpus](selfhost-frontend.md#limits-tests-and-remaining-work)
adds modules, opacity, lowering, core, effects and specialization on both adapters
and Chromium. The project sample also executes in native Wasmtime. Those Rust
reference tools are test-only.

Literal WAT GC programs require exact canonical output equality with the pinned
Rust assembler. `tools/selfhost/gc_contract.rs` validates both modules, preserves
GC recursive-group bytes and indices, and only normalizes trailing function-type
interning, local grouping and debug names. It never merges/reorders GC types,
removes instructions or folds unreachable code. High-level let/effect examples
are checked by execution and diagnostics rather than this literal-WAT contract.

ABI live-allocation counters and host handle tables must return to zero after
success and failure, also after forced GC. These counters cover ABI buffers and
host handles; they do not measure reclamation of every guest GC object. The
checks are regression evidence, not a general proof of leak freedom or validator
equivalence.

The [selfhost benchmark](selfhost-benchmark.md) freezes a scalar WAT corpus and
requires exact canonical-module equality with Rust before reporting timings.
It compares both guest adapters with the full Rust API and a WAT parser/validator
reference; every measured compilation must reproduce its captured raw bytes.
The saved timing and browser-size records predate GC and the modular frontend; measure a
fresh capture before claiming current performance or deployment size.

Next are full ownership/proof and ABI lowering,
remaining core features, floats/SIMD/multiple results, full recursive type
equivalence and memories/imports. Those constructs remain rejected until their
checks exist. The current subset's stage-1/stage-2 reproducibility gate is
implemented; it does not establish full Rust feature parity. See the
[stage contract](selfhost-stages.md), [frontend scope](selfhost-frontend.md) and [TODO](../TODO.md).


For request-local expansion counters, cache contracts and before/after artifact
comparisons, see [bootstrap frontend profiling](selfhost-profiling.md). This is a
separate diagnostic entry; the five production portable operations are unchanged.
