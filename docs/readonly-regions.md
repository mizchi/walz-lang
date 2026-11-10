# Shared Regions Immutable After Publication

This extends the fixed atomic-cell contract of `@race-free` with readonly ranges and checked loads. Under the assumption that initialization completes before sharing and that the host never changes contents after publication, Lean checks data-race freedom for traces of arbitrary length with any number of agents.

```wat
(module
  (@race-free)
  (import "env" "memory" (memory $shared 1 1 shared))
  (@atomic.cell $cancel $shared i32 0)
  (@readonly.region $index $shared 4096 8192)
  (func $read (export "read") (param $offset i32) (result i32) (@effect reads)
    (@readonly.load $index i32 $offset)))
```

`(@readonly.region $name $memory byte-offset positive-byte-length)` declares a fixed range within initial memory. `(@readonly.load $name u8/i32/i64/v128 relative-byte-offset)` reads 1/4/8/16 bytes from that range. The u8 result type is i32. It evaluates the address expression once, checks unsigned `relative <= length - width`, then loads. i32 bit patterns are unsigned offsets; accesses beyond the logical range trap. Access outside the declared region is rejected even when within the overall Wasm memory.

These are non-atomic loads, so natural address alignment is unnecessary. They have `reads` without implicit synchronization, blocking, or conversions. Ordinary `i32.load` or `v128.load` cannot bypass verification in this mode.

Files can use `(@export (region $index))` and `(@use "./layout.walz" (region $index $alias))`. A region name statically selects a load target; it is not a type extractable as a value or integer pointer.

## Structural Checks and Independent Kernel

A `@race-free` module with readonly regions checks the following:

- One backing shared memory with fixed limits. Raw shared-memory exports and host-function imports are rejected.
- Regions have positive lengths, lie within initial memory, and number at most 256. They cannot overlap atomic control cells; readonly regions may overlap one another.
- Ordinary shared-payload accesses use only checked readonly loads. Writes, raw loads, and writes to shared destinations through memory.copy are rejected.
- Unshared memory defined by a Worker is private and allows ordinary loads, stores, grow, and export. Importing private memory is not allowed.
- Public ABI functions have numeric parameters and numeric/unit results. GC objects stay within an agent.

Certificate version 2 includes readonlyRegions and readonlyAccesses in addition to atomic cells and accesses. Version 1 rules/proofs remain available. The kernel independently checks the load-opcode whitelist, width, region index, bounds, and non-overlap with control cells. The Readonly namespace in [AtomicCells.lean](../proof/AtomicCells.lean) proves `race_free` and `readonly_in_bounds` for traces including loads at permitted dynamic addresses. Tests reject forged stores, widths, region indices, overlaps, and versions.

Private-memory and GC operations are excluded from shared-payload traces. Wasm-engine memory isolation, the extractor, emission of unsigned guards and loads, initialization/publication order, and host compliance remain in the TCB or assumptions. This does not prove static freeze ownership for Builders, arbitrary JS code, or the entire Worker scheduler.

## ABI and Host Contract

Core+JSPI artifacts use `walz-core-js-v4` and `walz-readonly-regions-v1`. These versions prevent older runtimes from ignoring the new contract; value codecs are unchanged.

```js
const { api } = await instantiate({
  imports: { env: { memory } },
  readonlyHostContract: 'walz-readonly-regions-v1',
});
```

The host initializes before publication and never writes readonly regions afterward. It uses only same-width Atomics for control cells and does not reuse regions until every Worker has acknowledged completion. The selection string is neither JS-code verification nor OS memory protection. Direct raw Core instantiation requires the same assumptions.

`just test-succinct` checks the Core+JSPI host-contract handshake, logical range ends/negative offsets, address-expression evaluation counts, forged certificates, and execution with real Workers. Shared-memory import/export in Components remains unsupported. The parallel SDK uses Core Workers; standalone private-memory APIs target both Core+JSPI and Component.

Without `@race-free`, region declarations and checked loads are available, but immutability after publication and race freedom are not considered verified. This syntax does not prohibit writes in ordinary mode.
