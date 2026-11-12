# Type Checking and Static Analysis

The [dependency-graph workflow planner](../examples/apps/workflow/README.md) combines several features.
It implements a GC graph, state enums, assignment to multiple workers, cycle detection, and asynchronous I/O, and compares both backends against an independent reference implementation.

Annotations on function parameters, results, and similar boundaries determine the type of each expression, `@let` binding, and `case` payload.
`@let` also accepts an optional type annotation, as in `(@let $answer i64 (i64.const 42) body)`. There are no implicit numeric conversions.
Type errors report their location and source-level type names, for example `type mismatch: expected i64, got i32`.
Diagnostics also cover argument counts, name suggestions, the source of expected types, earlier moves, causes of effects, and the file that owns an opaque type.
The Rust API's `Error::diagnostic()` exposes the category, primary span, related locations, notes, and suggested correction. See the [diagnostic data contract](diagnostics.md).
For the CLI's `check --json` and `type-at`, which takes a line and column in the original file, see [inspection for external tools](tooling.md).

```sh
cargo run --locked -- check examples/ast-interpreter.walz
cargo run --locked -- types examples/ast-interpreter.walz
```

The typed core's `types`, `ir`, and `analyze` commands use name-resolved IR. The CLI's `check` also validates standard WAT.
`types` returns function result types, local IDs for params/captures, the types and declaration locations of every binding, and the types and source spans of every expression.
Local IDs distinguish bindings with the same name. Source spans are half-open intervals of UTF-8 byte offsets.

In the Rust API, `walz::check(source)` returns typed IR, and `walz::typing::report(&module)` provides the type listing.
`walz::typing::type_at(&module, offset)` returns the type of a binding declaration or the innermost expression.
Use `module.type_name(ty)` to format a returned type as notation such as `(ref $Expr)`.
Function signatures remain explicit; an omitted result means unit. Inference does not allow type annotations to be omitted throughout an entire program.
`walz::validate(source)` supports both standard WAT and extensions. When standard WAT lies outside the core, `types` displays function signatures from the validated Wasm; expression-level types are unavailable.
In that case, `Compilation.ir` is None, and analysis reports `opaque=true`, conservatively setting every module effect to true.

## Static Analysis

Analysis reports the following possibilities for each function and closure.

| Field | Meaning |
|---|---|
| `reads` | Reads explicitly mutable GC fields or elements |
| `writes` | Changes GC fields or elements |
| `allocates` | Allocates GC objects |
| `may_trap` | May trap because of null, bounds, integer division, allocation failure, and similar causes |
| `may_throw` | May throw a Wasm exception, including one caught inside the function |
| `may_suspend` | May logically suspend, allowing other work to intervene; conservatively true for unknown external calls |
| `may_diverge` | May fail to terminate because of loops, recursion, or external calls |
| `host` | Calls an external host function |

Call targets in the typed core are statically known, so effects propagate to a fixed point along the call graph, including recursion.
Reads of immutable fields do not contribute to `reads`; nullable references add `may_trap`.
`@match` conservatively adds `may_trap` because it includes runtime checks against invalid enum representations from external Wasm.
Unknown external functions conservatively set every field to true. Fixed event-loop bindings have their types and effects checked, propagating host and any required suspends effects. To execute host imports, the embedder must supply their implementations.
The CLI's `run` is a minimal launcher for an import-free `main`.

This analysis is a conservative approximation. It does not prove loop or recursion termination, index ranges, or the absence of allocation failure.
Diagnostic effect summaries do not authorize instruction elimination. Separate typed-IR passes eliminate [private value containers](value-optimization.md) and simplify [validated immutable ranges](range-optimization.md). Wasmtime/Cranelift handles subsequent code generation and optimization.
See [wasm-opt](wasm-opt.md) for the investigation of Binaryen as optional post-processing and the output strategy for large matches.

### Read/Write Targets and Facts from Branches

`just analyze examples/higher-order.walz` outputs memory and facts for each function in addition to the existing effect flags.
For map, memory.reads refers to the input parameter[0], and memory.writes refers to an allocation inside map.
The analysis also retains the fact that the output array has the same length as the input, establishing bounds_proven=true for both reads and writes.

| Output | Contents |
|---|---|
| memory.reads / writes | Targets of reads/writes to mutable GC fields or elements: parameter, capture, allocation, or unknown, with access paths |
| memory.returns | Origins of returned references and the internal representations of values |
| memory.allocations | Allocation types and locations, and possible escape through returned/stored/host/thrown paths |
| facts | Per-expression node, UTF-8 byte location, operation, reachability, i32 range, nullness, array length, and justification for bounds/null checks |

The analysis substitutes aliases for parameters, captures, and results across functions, tracking stored references, closures and value containers, cycles, loop/recur, and recursion.
It does not assume that arguments are distinct. Allocation sites merge across calls and iterations, and path components beyond depth 3 widen to unknown.
After suspension or unknown writes, information about references held in mutable fields widens.

The facts field i32_range is an unsigned bit interval. It accounts for wrapping and masked shift counts, falling back to the full range when an interval cannot represent the result.
It retains unsigned `i < array.len(a)`, signed comparisons with established non-negativity, null checks, and relationships between unchanged locals and array lengths.
Loop-carried values widen to their type's upper bound rather than retaining their initial values; branch joins keep only conditions common to both paths.
`bounds_proven=false` means "not established." Each fact concerns the normal continuation of that expression and does not rule out other traps such as allocation failure.

These diagnostic results do not relax existing pure/effect bounds or authorize removal of checks or allocations. The range optimizer uses its own must facts over checked IR, with separate conservative call/control-flow boundaries.
Non-escape alone also cannot justify removing reference identity or traps. i64 intervals and explicit loop invariants remain future work.
The Rust APIs are `regions::analyze(&module)` and `flow::analyze(&module, &function)`. `analysis::analyze` reports both.
