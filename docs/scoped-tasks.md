# Future/Stream Writers and Child Tasks

Writers use existing owned `@resource` values; readers use `walz:core` `$future<T>`/`$stream<T>`. The host's `createChannels()` provides Streams of capacity 0–65536 and Futures that settle once. No new general lifetime syntax is introduced.

The `pipeline` in `examples/channels/main.walz` connects a capacity-0 Stream to language-side producer and consumer tasks. It is tested, including `policy.json`, on both JS backends and Chromium.

```wat
(@use "walz:core" (type $task))
(@closure $Work (@capture $n i32) (result i32) (@effect pure) (+ $n 1))
(func $main (export "main") (result i32)
  (@effect host reads writes allocates suspends)
  (@task.scope (result i32)
    (@let $a (@task.spawn i32 $Work (@closure.new $Work 20))
      (@let $b (@task.spawn i32 $Work (@closure.new $Work 20))
        (+ (@task.join i32 (@move $a)) (@task.join i32 (@move $b)))))))
```

`@task.spawn T $Closure environment` starts a declared zero-argument closure returning `T` and returns an owned `$task<T>`. Captures use existing explicit declarations. Environments with owned captures require an explicit move. Result `T` must be shareable. Result GC objects preserve identity rather than being copied to JS.

`@task.join T handle` waits for completion and retrieves the result. `@task.drop T handle` waits for completion and discards the result. Both consume the handle; `drop` does not detach. Copying handles, leaving them unconsumed, exporting them through the ABI, or constructing them from arbitrary numbers is rejected.

`@task.scope (result T) body` waits for children on normal completion and exits through return, br, or exceptions. Each public function call also has an implicit parent scope. A child's failure cooperatively cancels its siblings and parent. Exceptional scope exit also requests parent cancellation. Return a `result` value for failures that should be handled. Cancellation completes after host operations, readers, and resources are released and the guest stack unwinds.

This is cooperative concurrency within one JS agent. It does not provide Worker parallelism, forced interruption of CPU loops, or select. Use `@task.cancel`/`@task.join-timeout` for child-only cancellation and deadlines. Closures whose inferred effects include event-loop-blocking operations such as `atomic.wait` cannot be spawned. Execution targets Core+JSPI or Component. The default backend of `walz run`, which executes standalone Core Wasm, has no scheduler.

## Writer Contract

`createChannels()` in `runtime/channels.mjs` is a host SDK; the concrete payload types in policy and Walz import signatures must match. Payloads transfer shareable ABI-compatible data by value. This differs from child-task results, which preserve GC reference identity.

| Operation | Contract |
| --- | --- |
| `stream(capacity)` / `future()` | Synchronous factories for owned writers, registered as resource `new` |
| `reader(writer)` | Acquires the reader once; the resource `reader` contract creates an independent owned endpoint |
| `send(writer, value)` | A Promise writing one Stream element; waits for capacity or a pending reader. 0=accepted, 1=closed/cancelled |
| `resolve(writer, value)` | Settles a Future once. 0=settled, 1=already closed. The reader is `future<result<T,i32>>`; closing before settlement produces err(1) |
| `close(writer)` | Synchronous and idempotent. A Stream drains accepted buffered values then reaches EOF; pending writes finish without retaining their values |

There is one reader and at most one pending write. Concurrent writes/reads are rejected. Closing a reader discards buffered values and wakes the pending writer. A write Promise's disposer cancels that write. Explicit writer close and `@using` destructors use existing resource management.

Owned values returnable from `@using` are extended only to Future/Stream/Task, which cannot contain borrows. For example, spawning a consumer inside `@using` and returning its Task allows joining outside after the writer closes. Returning borrows or other owned values remains prohibited.

## Implementation and Checking Scope

The private ABI between GC slots and the scheduler passes numeric IDs only. For pinned jco's Component output, `runtime/jco-children.mjs` contains the adjustment that runs children as separate threads within the parent task. A change in the original generation pattern causes build rejection. Tests check per-thread import association, deregistration on termination, and completion of parent Wasm stack unwinding.

Ownership reuses the existing Rust/Lean contracts. There is no machine proof yet of scheduler/JSPI/glue/finally semantics preservation or liveness. `just test-runtime-cli` and `just test-e2e` check concurrent-start barriers, reference identity, EOF, cancellation, sibling failure, and zero outstanding ABI allocations and GC slots.

## Deadlines and Host-Side Scopes

JS artifacts containing asynchronous exports also export `withTaskScope`.

```js
import {instantiate, withTaskScope} from './app/index.js';
const {start} = await instantiate();

const call = start('work', [], {timeoutMs: 1000});
await call.result; // 取消し・子タスク・host disposer の終了まで待つ

const result = await withTaskScope(start, async scope => {
  const first = scope.start('work', []);
  scope.start('anotherWork', []);
  return first.result;
}, {timeoutMs: 1000});
```

`timeoutMs` is an integer from 0–2147483647. Time counts from the start call, including time queued.
Zero cancels without entering the guest. Expiry produces `TaskTimeoutError`, a subclass of `TaskCancelledError`.
If the guest finishes first, a late deadline does not change its result. Timers and AbortSignal listeners are removed at termination.

One public call is the existing implicit parent scope. Expiry cancels that call and its Walz children,
rejecting result after child unwinding and resource release finish. It does not cancel other public calls.
Synchronous API return values and completion notifications including asynchronous disposers are distinct; use start's result to await complete release.

WithTaskScope groups its callback and public calls initiated through its scope.start.
Even normal completion waits for every call; callback/call failure, an external AbortSignal, or a deadline cancels member calls.
Scope.cancel(reason) also waits for release. Start after termination is rejected; a terminated scope retains no instance or past results.
Public calls on the same instance remain serialized. Use Walz child tasks for work that should progress concurrently.

This cancellation operates on public calls and host-side groups.
`@task.cancel T handle` consumes the handle and waits for cancellation, unwinding, and host disposal of that child and its descendants. The parent and siblings continue. `drop` waits for completion; `cancel` requests cancellation.

`@task.join-timeout T milliseconds handle` consumes the handle and returns `option<T>`: `some` on completion, or `none` after timeout-triggered child cancellation and release finish. The wait is a nonnegative i32, including 0. An already completed child's result takes priority. Ordinary traps and cleanup failures fail the parent rather than counting as success.

Cancellation is cooperative and cannot forcibly interrupt CPU work. Host operations must follow disposer and completion-notification contracts. Each scope tracks readers, owned endpoints, resources, ABI allocations, and pending disposals. Ownership of captures moved to a child also transfers. ABI return values live until post-return; cancellation reclaims only buffers of discarded children.

`proof/Tasks.lean` checks contracts preserving states outside the cancellation target and prohibiting acknowledgement before release. It does not mechanically verify the whole JS implementation. Arbitrary-scope cancellation tokens and select remain subsequent work.
Futures/Streams returned across the ABI transfer ownership to the host; deadlines do not extend through their lifetimes.
Callbacks and host operations must terminate cooperatively; a timer cannot forcibly stop CPU loops or unresponsive providers.

## Owned Timer Future

`walz:host/event-loop@0.1.0.after(i32)` is a fixed clock capability returning `future<unit>`.
It uses the same ownership, await, and close as existing Futures.

```wat
(@use "walz:core" (type $future) (type $unit))
(import "walz:host/event-loop@0.1.0" "after"
  (func $after (param $ms i32) (result (@inst $future $unit))))
(func $pause (export "pause") (param $ms i32) (@effect host suspends)
  (@future.await $unit (call $after $ms)))
```

Close/cancellation releases the timer. Types, effects, and capabilities are checked against the existing trusted binding.
