<a id="汎用の非同期workerホスト"></a>

# Generic asynchronous Worker host

[WorkerHost](../runtime/workers/host.mjs) provides one API for Node worker_threads and browser module Workers. It does not depend on succinct indexes, Wasm, or particular data formats. It handles startup, queues, delivery, cancellation, join, and shutdown, separating application-specific work into drivers.

```js
import { WorkerHost } from './runtime/workers/host.mjs';

await using host = await WorkerHost.create({
  workers: 4,
  driver: new URL('./my-driver.mjs', import.meta.url),
  init: { bias: 2 },
});

const result = await host.run({ numbers: [20, 20] });
const results = await host.map([
  { numbers: [10, 20] },
  { numbers: [40] },
]);
const task = host.start({ numbers: [1, 2] });
await task.result;
await task.cancel(); // 完了後なら何もしない
```

Write drivers as ordinary ES modules. Functions and closures are not stringified and sent to Workers.

```js
// my-driver.mjs
export async function create({ bias }) {
  return {
    async run({ numbers }, { signal, workerId, isCancelled }) {
      if (signal.aborted || isCancelled()) throw signal.reason;
      return { value: numbers.reduce((sum, n) => sum + n, bias) };
    },
    async dispose() {
      // このWorkerが所有する外部資源の終了を待つ。
    },
  };
}
```

`create` and `run` may be synchronous. `create` must return `run`; `dispose` is optional. `run` returns `{value, transfer?}`. To return a TypedArray without copying, use, for example, `{value: output, transfer: [output.buffer]}`.

<a id="apiと配送の規約"></a>

## API and delivery conventions

| API | Contract |
|---|---|
| `WorkerHost.create({workers, driver, init, cancellation?})` | 1–32 Workers; waits for every driver to initialize. Snapshots init once and passes the same cloneable value to each Worker |
| `run(payload, {signal?, transfer?})` | Assigns one job to an idle Worker and returns a result Promise; supports concurrent calls |
| `map(payloads, options)` | Queues jobs beyond the Worker count. Results follow submission order; the waiting queue holds at most 4096 jobs |
| `start(payload, options)` | Returns a `{result, cancel}` handle; `cancel(reason)` waits for running-job completion confirmation |
| `join()` | Waits for all jobs submitted as of the call, including errors and cancellation as completion |
| `close()` / `Symbol.asyncDispose` | Stops new submissions, cancels queued jobs, joins running jobs, and waits for each driver's dispose and Worker shutdown |

By default, job payloads undergo structured clone at delivery. Do not mutate payloads waiting in the queue. With `transfer: payload => [payload.bytes.buffer]`, the buffer transfers at delivery and becomes unavailable to the sender. Shared memory is shared without copying its contents. Wasm GC references, Promises, and functions are not delivery values.

`start().result` also has `Symbol.asyncDispose`. Passing this Promise to the existing Walz Future host adapter connects Future close to job cancellation and join. It does not send Futures themselves between Workers.

Drivers are trusted application code. The host neither verifies/isolates arbitrary JS nor automatically certifies arbitrary shared data as race-free.

<a id="取消しと共有メモリの寿命"></a>

## Cancellation and shared-memory lifetime

Async drivers receive a Worker-local `AbortSignal`. Cancellation messages request interruption, and caller Promises remain unsettled until `run` finishes. Cancelling one job/map group does not cancel other groups. The first cancellation/error reason is retained, including `0`, `false`, empty strings, and null.

CPU processing blocks the Worker's event loop, so messages alone cannot expose cancellation while it runs. Optionally specify the following.

```js
cancellation: { memory: sharedMemory, byteOffset: 0 }
```

This naturally aligned i32 cell becomes a host-exclusive cancellation bitmask. Atomics sets/clears only the Worker ID's bit. JS CPU loops use `context.isCancelled()`; Wasm periodically reads the same bit. Do not share the cell with other hosts/protocols. The generic host does not forcibly unwind functions at arbitrary positions.

The host retains the init snapshot and control cell until all execution finishes and driver disposal and Worker shutdown are confirmed. Cancellation requests, generation-number changes, and early Promise rejection alone do not permit shared-region reuse. Failed shutdown rejects close, retaining leases and permitting retry.

Browser drivers using SharedArrayBuffer/shared Wasm Memory require cross-origin isolation. This requirement is not imposed on ordinary Worker jobs that do not use shared memory.

<a id="walzとの接続"></a>

## Connecting to Walz

A [separate async Walz program](../tests/component-fixtures/worker-job.walz) and [example driver](../runtime/testing/worker-walz-driver.mjs) are provided. Each Worker instantiates Core＋JSPI or Component and awaits Futures within the Worker. Only the i64 result returns as BigInt. Cancellation is passed to the existing runtime's `start(..., {signal})`, waiting for cleanup to complete.

`@task.spawn` remains cooperative concurrency within the same agent. It does not implicitly send GC environments to another Worker. Worker data follows explicit clone/transfer/shared-memory contracts.

The [succinct driver](../runtime/succinct/driver.mjs) is one user of this generic host. Only the driver and QueryPool adapter know the index layout, Wasm reserve/batch functions, and output types; these details stay outside the generic host.

`just test-workers` checks async jobs, per-job cancellation of CPU work, transfer, waiting for all readers after cancellation, retries after shutdown failures, both Walz backends, and Chromium. This is not a formal proof of the entire JS host. See [readonly regions](readonly-regions.md) for the verification scope of shared-index data races.
