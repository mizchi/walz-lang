# Walz bootstrap codegen

English | [日本語](README.ja.md)

`compiler.walz` composes the Walz parser, [checker](../checker/README.md) and binary
emitter. `$compiler.compile(bytes)` returns Result of output bytes or a source
diagnostic. `$compiler.check(bytes)` validates without emitting bytes and returns
the function count. Parsing, checking and encoding run in Walz without a compiler
host callback.

`emit.walz` accepts only the checker's opaque module. `encoding.walz` provides
canonical unsigned 32-bit and signed 32/64-bit LEB128. `types.walz` emits GC
recursive groups, subtype definitions and value/storage types. Output sections
include GC/function types, functions, exports and code, including reference
locals/results, typed control and GC instructions. On the lower-level WAT path, GC indices and explicit recursive-group boundaries
are preserved without merging/reordering. The modular frontend first relocates
file-local indices and orders specialized types while preserving explicit group
membership and order.

```sh
just build-selfhost
just selfhost-check examples/selfhost/sum.walz
just selfhost-compile examples/selfhost/sum.walz target/selfhost-sum.wasm
just selfhost-compile examples/selfhost/gc.walz target/selfhost-gc.wasm
just selfhost-compile examples/selfhost/project/main.walz target/selfhost-project.wasm
```

The supported scalar subset includes unit/i32/i64 functions, mutable locals,
forward/recursive calls, arithmetic, blocks/loops, branches, if, immutable lexical
let and native exports. GC structs/arrays add packed fields, recursive groups,
declared subtypes, nullable references, casts and bulk fill/copy. Reference
matching is conservative rather than complete recursive type equivalence.
Unsupported forms are rejected. Syntax, IR and working
data use Wasm GC; immutable bytes cross the output ABI.

`compile-project`/`check-project` accept immutable file records. The
[frontend](../../docs/selfhost-frontend.md) adds static modules, fixed forms,
GC effect bounds, minimal core, bounded monomorphization and hygienic expression
macros with checked type queries.
Full ownership/proof/ABI lowering and stage 1/2 reproducibility remain future
work. This is not complete self-hosting. See the
[contracts, supported subset, limits and tests](../../docs/selfhost.md).
