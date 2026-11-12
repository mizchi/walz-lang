# Eliminating private value allocations

The typed-IR optimizer can replace local tuple/Option/Result containers with
their payloads in ordinary Wasm locals. This extends immediate constructor/match
simplification to local bindings, unused values, multiple matches, branches,
and small helper calls. No new syntax, unchecked operation, or runtime is added.

See [the value pipeline example](../examples/value-pipeline.walz): a helper
returns `result<tuple<i32,i32>,i32>`, using a checked-multiplication Option inside.
The caller decomposes it into an i32 result. The optimized pipeline needs no
intermediate private GC containers.

```text
let value = choose(flag, n)       // returns none or some(n)
match value { none => 0, some(x) => x }
  + match value { none => 1, some(x) => x + 1 }
```

For an eligible helper this becomes one evaluation of its arguments and branch
condition, with scalar payload locals and the selected continuation. It does not
call `choose` twice or reevaluate the payload for each match. Actual source uses
the existing `@let`, `@match`, and `call` forms.

## Conditions and semantics

- Only private, shared tuple/Option/Result storage is eligible. The container must
  be constructed in the checked function or exposed by bounded helper inlining.
  Every use of an eligible local must be a match or drop; unused locals qualify.
- Payload expressions run once, left to right, at their original construction
  point, before the continuation. This includes unused payloads and `@default`
  cases. Their ordinary object allocations, mutations, host effects, suspension,
  exceptions, and explicit traps remain.
- Ordinary GC payload references retain identity and aliasing. None stays distinct
  from some(null) and some(unit); Result's two unit cases remain distinct too.
- Fusion can pass through producer `let`, block, if, and shared match expressions.
  It never moves a consumer into a producer's try/catch, labeled block, or loop
  scope. Rewrites within those scopes can still apply independently.
- All source type/effect/ownership/opaque/proof checks precede optimization, and
  inspection keeps the original checked IR. Diagnostic escape facts do not
  authorize allocation removal.

Private carrier allocation has no source-observable identity. Its physical
allocation and corresponding OOM point can disappear. Allocation timing and GC
timing are not preserved; explicit payload traps, null/bounds checks on remaining
operations, and externally observable reference identity are preserved.

## ABI and conservative limits

Public function types and Core/Component/experimental JS adapters retain their
existing boxed representations. Escaping values, fields, arrays, closure captures,
unresolved aliases, and unknown calls retain their containers. External values
still need runtime tag and subtype validation, including default cases. Merely
declaring an opaque type never proves that a foreign value is well formed.

Functions containing owned/resource/borrow locals or results are excluded.
Identity-bearing user enums, ordinary structs/arrays, closures, Own, Future,
Stream, and Task storage are not scalarized by this pass. Helpers with captures,
explicit returns, exception scopes, recursion during expansion, or excessive size
remain calls. Loop-carried value representations and producer loops remain boxed.

Inlining and continuation duplication have fixed work/code-growth budgets. A
failed rewrite restores the original subtree and locals; inlined function copies
are retained only when some private allocation was removed. Nested cases receive
up to four passes, so elimination is intentionally incomplete. Code size and speed
are workload dependent; allocation elimination alone is not a speed guarantee.

Wasm multi-value function specialization was considered. For this bounded scope,
consumer fusion removes the carrier without changing call signatures or creating
another calling convention. General multi-value returns for large/recursive
helpers and loop-carried containers remain future work. `CompileOptions {
optimize: false }` retains the original lowering.

## Checks

```sh
just test-value-optimization
pnpm exec playwright test runtime/e2e/value-optimization.spec.mjs
cargo run --locked -- run examples/value-pipeline.walz
```

Rust tests inspect allocation/cast removal and compare optimized/unoptimized raw
Wasm on boundary bit patterns, host event traces, default payloads, exceptions,
and forged base values. A budget regression verifies that a declined expansion
leaves the original WAT intact. Node compares checked layouts to unbounded BigInt
on four JS modes, and tests suspension, rejection, cancellation, and ABI cleanup
on Core+JSPI and Component. Chromium covers both backends.

This optimizer is compiler TCB. Existing Lean ownership contracts still check the
source; they do not prove semantic preservation of these rewrites or the emitted
Wasm. See [verification boundaries](../proof/README.md).
