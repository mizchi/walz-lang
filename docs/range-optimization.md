# Optimizing validated immutable ranges

The typed-IR optimizer can carry range checks through small shared-value helpers
and into guarded loops. No source annotation, unchecked access API, runtime state,
or change to `pure` is required. The ordinary type, effect, ownership, visibility,
and contract checks run before optimization.

See [the complete example](../examples/range-scan.walz), which sums little-endian
signed i32 words in an immutable bytes view and returns 42 from `main`.

```wat
(@let $length (call $bytes-view.len $v)
  (@loop $scan (bind $at i32 0) (bind $total i64 0) (result i64)
    (if (result i64) (i32.lt_u $at $length)
      (then
        (if (result i64) (i32.le_u 4 (- $length $at))
          (then (@recur $scan (+ $at 4)
            (+ $total (i64.extend_i32_s (call $bytes-view.read-i32-le $v $at)))))
          (else $total)))
      (else $total))))
```

Here `at <_u length` establishes that subtraction cannot underflow. The second
guard establishes that the whole four-byte word fits. A validated view construction
also establishes that its range fits the immutable owner. When construction and
reading are exposed by bounded inlining, these facts compose: the reader needs
neither another logical view-width guard nor its two explicit backing-buffer
range guards. **Wasm GC `array.get_u` bounds checks remain.** This is not a promise
of one machine load or a measured speedup.

## What counts as evidence

The optimizer has a separate must-fact representation. It does not consume the
diagnostic `memory` / `facts` report or its `bounds_proven` flag.

- Unchanged local snapshots, immutable struct fields, array lengths, and known
  immutable constructions retain their relationships.
- Unsigned guards establish `start <= limit` and `width <= limit - start` on
  the appropriate normal branch. Branch joins retain only common evidence.
- A successful checked read, slice, or ranged UTF-8 operation establishes bounds
  for its normal continuation. A later read of the same range can reuse them.
- Subranges compose only after proving each complete range fits. This also proves
  that the translated offset does not wrap.
- Each loop trip starts with unknown loop-carried values. Guards must establish
  the range for that trip; the initial value is not an invariant. Immutable
  preheader snapshots remain available, and body facts do not escape the loop.

When a branch condition is already established, the optimizer still evaluates
the original condition and drops its value before executing the chosen arm.
Operand effects, exceptions, and nullable-reference traps are preserved. Read
operands also keep their once-only, left-to-right evaluation order.

## Conservative limits

Only a bounded subset of small shared-value expression helpers is inlined.
Fresh locals and loop labels avoid collisions. Recursive expansion, captures,
owned/resource/borrow values, and unsupported control-flow scopes remain calls.
Unknown calls discard range facts. Exception, match, label, and other unsupported
constructs do not export facts from their children. Mutable-field reloads are
fresh values. No invariant is assumed merely from an opaque type or public Wasm
parameter, and different arguments are not assumed distinct.

Signed comparisons and wrapped `start + width <= limit` alone do not authorize
check removal. General interprocedural contracts, signed/i64 range inference,
loop invariants, and GC engine check elimination are outside this pass. Analysis
and inlining budgets conservatively retain checks. Inlined bodies are kept only
when a guard or read is actually simplified; code size and speed remain workload
dependent. `CompileOptions { optimize: false }` disables typed-IR simplification.

## Reproduce the checks

```sh
just test-range-optimization
pnpm exec playwright test runtime/e2e/range-optimization.spec.mjs
cargo run --locked -- run examples/range-scan.walz
```

Rust tests inspect the emitted proven helper and compare optimized/unoptimized
raw Wasm with boundary inputs, host effects, null traps, mutable reloads, and
loops. Node tests compare an independent DataView model on Core+JSPI, Component,
and both experimental direct JS modes, including nested logical bounds and ABI
cleanup. Chromium tests cover Core and Component.

[Ranges.lean](../proof/Ranges.lean) proves the unsigned arithmetic rules and the
nonwrapping composition step. It does not prove the Rust fact extractor,
inliner, optimizer, or Wasm lowering; those remain compiler TCB.
