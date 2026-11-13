# Walz bootstrap checker

English | [日本語](README.ja.md)

The checker is written in Walz and runs without a compiler host import. It covers
the bootstrap compiler's unit/i32/i64 and GC subset, not all Wasm or the full
Walz language.

- `ir.walz` defines untrusted immutable GC type/field/instruction/function/export records.
- `operators.walz` defines closed scalar opcode, type, arity and alias tables.
- `gc-operators.walz` supplies the GC opcode table; `syntax.walz` shares syntax access.
- `type-env.walz` resolves source types, fields and recursive groups.
- `gc-types.walz` validates raw GC descriptors, field variance and reference subtyping.
- `checker.walz` resolves source names, infers types and lowers syntax.
- `stack.walz` independently validates operand/control stacks, signatures,
  indices, opcode immediates, export names and resource limits.
- `gc-effects.walz` unions mutable GC effects and checks pure/reads/writes/allocates
  bounds, propagating direct-call effects to a fixed point.

`$checker.check(syntax)`, `$checker.validate(functions, exports)` and
`$checker.validate-gc(types, functions, exports)` return
`$result<(ref $module-ir), (ref $diagnostic)>`. Only successful validation creates
the opaque checked module accepted by the emitter. Public raw records and
read-only checked-module getters cannot mutate a validation result.

The source subset includes mutable locals, labelled blocks/loops, branches,
return, if, select and single-binding immutable lexical let. Unreachable code is
still validated using Wasm's stack-polymorphism rules. GC structs/arrays support
packed fields, recursive groups, declared subtypes, nullable references, casts
and bulk array fill/copy. Immutable construction/reads are pure. Non-null lets
use nullable physical local storage with checked lexical initialization.

Reference matching follows declared indices and parent chains; full recursive
type equivalence remains future work, so some valid Wasm is rejected. Floats,
SIMD, imports, function/external references and full effect/ownership/proof
checking remain unsupported. The separate [frontend](../../docs/selfhost-frontend.md)
adds static modules, fixed forms, minimal core, bounded type/function generics and hygienic macros with independently
validated type queries
before independent validation. Declared locals must be
defaultable; non-null locals await initialization analysis.

```sh
just build-selfhost
just selfhost-check examples/selfhost/sum.walz
just selfhost-check examples/selfhost/gc.walz
just selfhost-check examples/selfhost/project/main.walz
just test-selfhost
```

See [API contracts, source syntax, diagnostics and differential checks](../../docs/selfhost.md).
