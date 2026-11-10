# Shared Linear Memory and Atomics

Following the [Wasm threads specification](https://github.com/WebAssembly/threads/blob/main/proposals/threads/Overview.md), the host creates and joins threads. Walz checks shared-memory access in typed IR and emits WAT instructions directly. No custom thread runtime or messaging between GC heaps is added.

## Using Host Memory

`counter.walz`:

```wat
(module
  (import "env" "memory" (memory $shared 1 2 shared))
  (func $add (export "add") (param $address i32) (param $n i32) (result i32)
    (@effect reads writes synchronizes)
    (i32.atomic.rmw.add $shared $address $n)))
```

```sh
node runtime/build-core.mjs counter.walz target/counter
```

```js
const { instantiate } = await import('./target/counter/index.js');
const memory = new WebAssembly.Memory({ initial: 1, maximum: 2, shared: true });
const { api } = await instantiate({ imports: { env: { memory } } });
const words = new Int32Array(memory.buffer);
api.add(0, 1);                 // 変更前の値 0
Atomics.add(words, 0, 2);       // 同じ領域の変更前の値 1
api.add(0, 0);                 // 3
```

Pass the same `memory` to Workers through structured clone and import it in each Worker. Memory contents are not copied. Core+JSPI ABI temporary storage uses separate unshared memory, so host memory is not subject to allocation or cleanup. Executable examples are in the [Node tests](../runtime/threads.test.mjs) and [browser Worker tests](../runtime/e2e/threads.spec.mjs).

Typed declarations require `$name` and memory32 page counts. Each page is 64 KiB, with a limit of 65,536 pages. `shared` requires a maximum page count, with initial no greater than maximum. The Wasm engine validates sharing attributes and minimum/maximum page counts on host imports. Atomics can also use memory without `shared`, but wait traps at runtime.

Definitions such as `(memory $private 1 2 shared)`, inline `(export "memory")`, and `(export "memory" (memory $shared))` are also supported. File libraries can select/alias memory with `(@export (memory $mem))` and `(@use "./counter.walz" (memory $mem $shared))`. Private memories cannot be referenced externally; DCE removes unused memory imports.

## Instructions and Types

The 67 instructions in the original threads proposal are supported. Instruction names have no `@` prefix.

| Instruction family | Inputs other than address | Result |
| --- | --- | --- |
| `i32/i64.atomic.load`, `load8_u` / `load16_u` / `i64.atomic.load32_u` | None | i32 / i64 |
| `i32/i64.atomic.store`, `store8` / `store16` / `i64.atomic.store32` | i32 / i64 | unit |
| `i32/i64.atomic.rmw[8/16/32].{add,sub,and,or,xor,xchg}[_u]` | i32 / i64 | Previous value |
| `i32/i64.atomic.rmw[8/16/32].cmpxchg[_u]` | Expected/replacement values | Previous value |
| `memory.atomic.wait32` | i32 expected value, i64 timeout | i32 |
| `memory.atomic.wait64` | i64 expected value, i64 timeout | i32 |
| `memory.atomic.notify` | i32 notification count | i32 |
| `atomic.fence` | None; no memory operand either | unit |

Addresses are byte-based i32 values. Narrow loads/RMW zero-extend, while stores write low bits. There is no narrow 32-bit i32 form or narrow 64-bit i64 form; use the ordinary form. Narrow RMW forms require `_u`.

`memory.size $mem` returns the page count. `memory.grow $mem pages` returns the previous page count, or -1 on failure. Host code using the length after growth must recreate its view from `memory.buffer`.

```wat
(i32.atomic.load $shared offset=8 align=4 (i32.const 0))
(i64.atomic.rmw.cmpxchg $shared 16 (i64.const 0) (i64.const 1))
(memory.atomic.wait32 $shared 0 1 (i64.const 1000000))
(memory.atomic.notify $shared 0 1)
(atomic.fence)
```

Offset is an unsigned memory32 immediate. Align must equal the access width in bytes. Misaligned or out-of-bounds actual addresses follow Wasm trap rules. All operations are sequentially consistent; there is no custom memory-order syntax.

At the root, omitting the memory name/index selects memory 0. Walz literal shorthand is distinguished by input count: `(i32.atomic.load 0)` means address 0; `(i32.atomic.load $shared 0)` specifies memory and address. File modules must name memory explicitly so adding another file cannot change the target. Anonymous memories, memory64, non-atomic loads/stores, and shared GC/shared-everything threads are outside these typed additions; the validated standard WAT path remains available.

## Effect and Ownership Contracts

| Operation | Semantic effect |
| --- | --- |
| Atomic load | `reads synchronizes` |
| Atomic store | `writes synchronizes` |
| Atomic RMW / cmpxchg | `reads writes synchronizes` |
| Wait32 / wait64 | `reads synchronizes blocks` |
| Notify / fence | `synchronizes` |
| Shared-memory size / grow | `reads synchronizes` / `reads writes synchronizes` |

`synchronizes` means synchronization order is observable; `blocks` means the current agent may be stopped synchronously. These propagate from callees and are outside a `pure` bound. `blocks` does not add JSPI `suspends`. Analysis also reports `synchronizes`/`may_block`.

Memory is a shared capability referenced by the module, not an owned value. Existing `@where (shared $T)` means copyable within the same agent, not transferable to Workers or Rust Send/Sync. Storing GC `ref`, `box`, resource, or Future/Stream endpoints as atomic values is a type error. Integer addresses/values are copyable. Expression arguments evaluate once each from left to right; moves, double consumption, unconsumed values, and return paths within them undergo existing ownership checking. Local owned values stay in their thread while waiting.

Ordinary atomics-mode checks do not prove address-specific exclusive ownership, prohibition of region access after sending, freedom from host-code races, lock correctness, or deadlock freedom. No rule treats an i32 pointer as a type owning its region. In ordinary mode, the Lean kernel checks the existing ownership contract. For a conditional proof restricted to fixed cells, use `@race-free` from [atomic-cell verification](atomic-cells.md). Sending objects across threads requires a separately defined integer/byte protocol and lifetime/synchronization rules.

Wait timeouts are nanoseconds; negative values mean indefinite waiting. Results are 0=notified, 1=not-equal, and 2=timed-out. They share queues with JS `Atomics.wait`/`waitAsync`/`notify`. Synchronous wait is unavailable on browser main threads. Workers need finite timeouts or explicit notify-based termination. Task cancellation does not interrupt synchronous wait.

## Boundaries and Tests

[WIT types](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md) have no raw-memory type. This implementation rejects Component/WIT output importing/exporting memory with `E_COMPONENT_MEMORY`. Memory defined and used internally in a Component is allowed. JS conversion uses jco `multiMemory: true`, passing multi-memory directly to the engine. Raw memory exports are available through Core Wasm `instance.exports`. Generated Core+JSPI APIs expose only functions; the host retains the backing memory.

Ordinary Core `--verified` rejects host memory imports with `E_VERIFIED_MEMORY`. `@race-free` accepts imports checked under the fixed-cell contract, but runtime execution still requires host-provided memory and contract compliance.

Serving browser shared-memory applications requires cross-origin isolation. The test server supplies COOP `same-origin` and COEP `require-corp`. The Playground does not supply host memory.

```sh
just test-threads
```

Tests cover types and generated Wasm for all 67 instructions, natural alignment, argument types, effects, owned values, DCE, sharing with the host, concurrent Worker increments, JS ↔ Wasm wait/notify, and private memory in Core/Component. The instruction family itself requires no thread pool.

The [general Worker host](worker-host.md) is a separate explicit library. It dispatches asynchronous and CPU jobs on both Node and browsers and manages individual cancellation, join, and shutdown. [Shared regions immutable after publication](readonly-regions.md) and [succinct indexes](succinct.md) demonstrate its use.
