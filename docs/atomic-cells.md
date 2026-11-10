# Conditional Race-Freedom Verification for Fixed Atomic Cells

A module marked `@race-free` restricts shared linear-memory accesses to statically declared atomic cells. In addition to Rust's type, ownership, and effect checks, compilation runs an independent Lean kernel. This excludes shared-memory data races under the assumption that the host follows the same contract.

```wat
(module
  (@race-free)
  (import "env" "memory" (memory $shared 1 1 shared))
  (@atomic.cell $counter $shared i32 0)
  (func $add (export "add") (param $n i32) (result i32)
    (@effect reads writes synchronizes)
    (i32.atomic.rmw.add (@atomic.at $counter) $n)))
```

An executable [fixture](../tests/component-fixtures/atomic-cells.walz) and [Worker example](../runtime/atomic-cells.test.mjs) are included.

This page describes the version 1 contract, which uses only atomic cells. [Readonly regions in version 2](readonly-regions.md) add shared payloads that remain immutable after publication and private memory for each Worker. The [general Worker host](worker-host.md) supports scheduling and cancellation.

## Checked Rules

- There is at most one backing shared memory. It must be `shared`, have equal initial and maximum page counts, and have no raw memory export. Memory-free computation and fences are also accepted.
- `(@atomic.cell $name $memory i32/i64 byte-offset)` declares a fixed cell. Checks require natural alignment within initial memory, non-overlapping cells, and at most 256 declarations. Declarations do not write initial values.
- `(@atomic.at $name)` is allowed only in atomic address positions. It cannot be extracted as an integer or reference, used in addition/subtraction or casts, or passed as a function argument. Omitting the memory selects the cell's memory; an explicit memory must match it.
- Full-width i32/i64 load, store, RMW, and cmpxchg, waits of the same type, notify, and fence are supported. Narrow accesses, extra offsets, raw addresses, and size/grow are rejected. Notify operates on the cell's wait queue and is not treated as a payload write.
- Checks include unused definitions. Unknown host-function imports and non-numeric ABI parameters/results are rejected; unit results are allowed. GC data stays within an agent, preserving the existing move/consumption rules.

`@atomic.cell` and `@atomic.at` can also be used as static typed sugar without `@race-free`, but are not treated as verified in that case. Ordinary WAT and existing atomics with arbitrary addresses remain supported.

Files can select cells with `(@export (cell $counter))` and `(@use "./state.walz" (cell $counter $alias))`. Aliases refer to the same cell. Put `@race-free` in the entry module; it applies to all included definitions.

## Host Contract

The generated Core+JSPI API requires an explicit contract selection for shared-memory imports.

```js
const memory = new WebAssembly.Memory({ initial: 1, maximum: 1, shared: true });
const { api } = await instantiate({
  imports: { env: { memory } },
  atomicHostContract: 'walz-atomic-cells-v1',
});
Atomics.add(new Int32Array(memory.buffer), 0, 1);
api.add(2);
```

`raceFree.cells` in `manifest.json` describes the `{offset, width}` layout. The host and every Worker/Wasm instance using the same memory must follow these rules:

1. After sharing begins, access payloads only through Atomics/Wasm atomics matching the declared cell ranges and widths.
2. Do not use ordinary TypedArray writes, overlapping accesses of different widths, or reinitialization through another module's data segments.
3. Use declared cell addresses for wait/notify. Control termination with finite timeouts or explicit notify operations.

Verified Core artifacts use `walz-core-js-v3`. Value codecs are the same as v2; v3 requires a contract descriptor and explicit host selection. Older runtimes reject this as an unsupported ABI, preventing execution that skips contract acknowledgement. Ordinary artifacts continue to use v2.

Initialization before sharing is outside the model and treated as an arbitrary initial state. The Wasm engine checks memory sharing attributes and limits. Without a contract selection, the API rejects the request before instantiation.

**Selecting a contract does not verify host code.** If the host selects it but violates the rules, the proof does not apply. Direct instantiation of raw Core Wasm requires the same assumption. Private memory needs no external host contract selection and works with both Core and Component.

## What Lean Proves

[AtomicCells.lean](../proof/AtomicCells.lean) independently checks the cell layout and access-instruction list extracted by the compiler. Lean also has its own opcode whitelist, rejecting non-atomic instructions, mismatched widths, and accesses outside the layout.

For a contract that passes checking, it proves the following for any number of agents and execution traces of arbitrary length.

| Theorem | Guarantee |
| --- | --- |
| `race_free` | Conflicting accesses between agents never include a non-atomic access |
| `uniform_cells` | Overlapping payload accesses have the same start and width |
| `in_bounds` | Every access is 4/8 bytes, naturally aligned, and within the declared memory |

The model lets any agent execute any checked instruction site any number of times. It overapproximates control flow without bounded unrolling of execution counts. Host events follow the same cell contract. Notify, fence, and intra-thread GC operations are stuttering steps in the payload trace.

Source parsing, name resolution, IR extraction, optimization, DCE, Wasm emission, the engine, ABI implementations, and host compliance remain in the TCB or assumptions. This is not a proof of semantics preservation for all generated Wasm or formal verification of arbitrary JavaScript.

Even without data races, `atomic.load → addition → atomic.store` can lose updates. The Node test pauses two Workers immediately after both loads and reproduces a final value of 1 when both write 1. With `rmw.add`, the additions are preserved in the same execution. This version does not guarantee linearizability, deadlock freedom, fairness, or splitting/moving exclusive regions `Owned<R>`.

## Commands and Evidence

```sh
cargo run --locked -- race-contracts tests/component-fixtures/atomic-cells.walz
cargo run --locked -- verify tests/component-fixtures/atomic-cells.walz
just test-atomic-cells
```

`check` and `race-contracts` perform only Rust structural checks and contract extraction. `build`, `wat`, `core-js`, `component`, and `verify` also require Lean checking of the target code to succeed. The in-browser compiler cannot run Lean and rejects this mode with `E_HOST_UNAVAILABLE`. Artifacts verified and generated natively can run in a browser.

Design decisions and evidence:

| Item | Details |
| --- | --- |
| Contract origin | The user's choice to restrict accesses to fixed-width atomic cells and assume a host contract |
| Implementation | Lowers cell references to constant addresses and checks memory instructions in all functions. Verified mode prohibits opaque fallback |
| Machine checking | Lean trace invariants and three theorems. Rust/Node/browser tests compare acceptance, rejection, and execution |
| Rejected witnesses | Partially overlapping i64/i32 cells, i32.store, different widths, and offsets outside the contract. The kernel also rejects tampered JSON |
| Retained witness | Lost updates in a load/store protocol whose operations are all atomic. Distinguishes a data-race proof from logical races |
| Host boundary | Retains a counterexample with a non-atomic host write in the model. The guarantee fails without the host contract |
