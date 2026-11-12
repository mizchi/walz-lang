<a id="future--stream-と-javascript-実行"></a>

# Future / Stream and JavaScript execution

```wat
(@let $future (@future.ready i64 (i64.const 42))
  (@future.await i64 (@move $future)))
```

`(@inst $future T)` and `(@inst $stream T)` are opaque owned values. Construct them with ready/empty/cons and access them only through await/next/close.
next returns `(@inst $tuple (@inst $stream T) (@inst $option T))`, so use the returned Stream on the next read. Even terminal none returns an endpoint that must eventually be explicitly closed.
Payloads are restricted to shareable data, including unit, Result, and GC references. The initial version rejects Future/Stream themselves and own values as payloads.
await/next have suspends, which propagates to callers. WIT future/stream types and async reading functions are supported too.

[ready-delivery.walz](../examples/ready-delivery.walz) sums 40 and 2 from a stream, checks a separate completion Future's Result, and returns 42.
Ordinary core Wasm output stores already-delivered data in GC; reads return immediately.
Component output can also hold external readable handles, with await/next waiting through JSPI for Promise/AsyncIterable delivery.
close consumes the endpoint and invokes Canonical ABI drop-readable for external endpoints. The fixed host implementation releases timers that are no longer needed.
Task cancellation from the JS runtime and cancellation of the associated in-flight read are supported. Shared-payload [writers and scoped spawn / join](scoped-tasks.md) also express producers/consumers within the language.
For standalone CLI distribution, use `walz build --target core-js`; for direct execution, use `walz run --target core-js`. Explicitly supplied files connect through [WASI 0.3 value adapters](wasi-files.md). See the [CLI conventions](cli.md).

<a id="component-abi-と-javascript-のイベントループ"></a>

## Component ABI and the JavaScript event loop

```sh
pnpm install --frozen-lockfile
just component               # target/component.wasm と .wasm.json を生成
just build-js                # jco で JSPI 用の JS と core Wasm に変換
just run-js                  # タイマーを待ち、GC 内の値を使って → 42
node runtime/run.mjs target/event-loop sumTicks 5 1  # Stream の総和 → 10
just test-js
pnpm exec playwright install chromium
just test-e2e
```

Use Node.js 24 or later and pnpm in addition to Rust/Lean. Verified with Node 24.21.0, pnpm 12.11.2, jco-transpile 0.23.0, and Chromium 156 in Playwright 1.64.1.
The runtime requires Wasm GC, exceptions (try_table), and [JSPI](https://v8.dev/blog/jspi) `WebAssembly.Suspending` / `WebAssembly.promising`.
[jco](https://bytecodealliance.github.io/jco/transpiling.html) bridges the Canonical ABI and JSPI, allowing JavaScript timers to progress while waiting. Public calls on the same instance run sequentially; child tasks within one call run cooperatively in parallel.

[event-loop.walz](../tests/component-fixtures/event-loop.walz) implements a main retaining GC references across waits, an asynchronous Stream sum, branching on a completion Result, and closing a pending Future.
The fixed host contract is `walz:host/event-loop@0.1.0`.

| import | Type | Effects |
|---|---|---|
| sleep | `(i32) -> unit` | host, suspends |
| yield | `() -> unit` | host, suspends |
| defer-i64 | `(i32, i64) -> future<i64>` | host |
| ticks | `(i32, i32) -> stream<i64>` | host |
| fallible-i64 | `(i32, i32) -> future<result<i64,i32>>` | host |

Times are in milliseconds. ticks takes an element count and interval and delivers values starting at 0. fallible-i64 delivers ok(42) for code=0 and err(code) otherwise.
Public functions require effects contracts. Component generation also checks the same IR's ownership contracts and laws with Lean. Unknown imports, GC references or own at boundaries, and escaping exceptions are rejected.

Internal values and ABI buffers are separate, with conversion for numbers, Option, Result, pair, Future, and Stream. Future/Stream use the Component Model's own types and readable handles.
Buffers are allocated per call. Return values are released at post-return; arguments and reads are released after conversion. The arena limit is 16 MiB.
`walz:runtime/delivery@0.1.0` is an internal import for moving ready values and prefixed Stream elements to JS delivery endpoints. Types and factory names are generated per module, so distribute `.wasm.json` and the matching JS host together.
`walz wit` emits the logical public API shape; `wasm-tools component wit target/component.wasm` emits the actual Component contract, including internal imports.

Generated JS receives localized fixes for the pinned jco version's unit/option.none delivery and unresolved Promises from failed imports. These fixes are collected in [jco-compat.mjs](../runtime/jco-compat.mjs); runtime tests check delivery in both directions and failures.
Return application failures through Result. Invalid timer arguments trap, leaving the instance unusable.
This currently implements custom timer bindings, rather than wasi:* file/network APIs or the full WASI 0.3.0 suite.

<a id="軽量な-corejspi-出力"></a>

### Lightweight Core＋JSPI output

```sh
just build-core-js                  # target/core-event-loop に生成
just run-js target/core-event-loop  # → 42
just test-core-js
just memory-core-js
```

A dedicated JS output is also available, informed by [taraba](https://github.com/mizchi/taraba).
`runtime/build-core.mjs` emits two core Wasm modules, a small shared host, and generated type conversions without loading the Component transpiler.
`instantiate()`'s `api` / `start` and event-loop bindings are shared with the path above. Synchronous functions remain synchronous; Future/Stream ownership and cancellation completion checks are preserved.
The Wasm engine manages GC values and suspended stacks; the host manually manages only temporary buffers and external endpoints.

This path uses ABI `walz-core-js-v2`. For standard Component interoperability, use `just component` / `just build-js`.
Codecs are generated from concrete public API and delivery-payload types, without interpreting type schemas at runtime.
See [SPEC.md](../SPEC.md#corejspi-の-js-専用-abi) for API restrictions and the internal ABI, and [MEMORY.md](../MEMORY.md) for size and memory comparisons.

`just profile before` / `just profile after` save line-filter guest profiles and execution metrics;
`just profile-compare before after` remeasures saved artifacts in alternating order.
[PROFILE.md](../PROFILE.md) records the change reducing ABI allocations by about 19%, UTF-8 validation tuning, and measurement conditions.

The [log aggregation CLI](../examples/apps/log-stats/README.md) is a somewhat larger practical example.
It uses static modules and the string library for TSV parsing, GC aggregation, integer-to-string conversion, and one-line JSON output.
Result / Stream macros express ownership-preserving iteration, with aggregation state exposed as an opaque type.
After `just build-log-stats`, run `just log-stats input.tsv` and check with `just test-log-stats`.
`just profile-log-stats` saves a guest profile of the shared computation and actual Core＋JSPI execution time.

The [inventory ledger CLI](../examples/apps/stock-ledger/README.md) exercises the improved features on another workload.
`just build-stock-ledger` → `just stock-ledger` emits JSON Lines from Unicode item names and quantities.
The handwritten and macro versions are compared using the same operation sequence; `just audit-stock-ledger` compares source volume, generated IR, and execution time.

<a id="utf-8-行フィルタと-io-capability"></a>

### UTF-8 line filter and I/O capabilities

```sh
just build-text
printf '日本語🦀\n\nlast\n' | just text-filter
just text-file input.txt output.txt
just test-io
```

The [line filter](../examples/io/text-filter.walz) outputs input with empty lines removed and returns the number of output lines as `result<i32,i32>`.
Input supports LF / CRLF separators, missing final newlines, and UTF-8 split across chunks. Each output line ends in LF.
CLI stdout contains only processed data. Exit codes are 0 for success, 1 for I/O failure, and 130 for SIGINT cancellation. Specifying the same file for input and output is rejected.

The fixed contract is `walz:host/text-io@0.1.0` in [text-io.wit](../interfaces/text-io.wit).

| import | Type | Required capability |
|---|---|---|
| read-lines | `() -> stream<result<text,i32>>` | text-input |
| write-line | `(text) -> future<result<unit,i32>>` | text-output |

Both have the host effect. Calls only return endpoints; reads and waits for completion have suspends.
The compiler checks import names, signatures, and effects, recording required capabilities and types in manifest `imports`.
Embedders explicitly supply the host with `instantiate({imports: {'walz:host/text-io@0.1.0': io}})`. Instantiation is rejected if required functions are missing.
`createTextIO({input,write})` in `runtime/text-io.mjs` connects a byte iterable and `write(bytes,AbortSignal)` to the existing value types.
`runtime/node-io.mjs` handles Node streams and file opening/closing; `runtime/text-cli.mjs` handles the CLI.
These I/O implementations are not automatically included in ordinary Core output. The host controls file paths and standard input/output selection.

I/O error codes are 1 for invalid UTF-8, 2 for read failure, 3 for write failure, 4 for line-length limit, and 5 for closed input or aborted writes.
Input can be consumed only once; the default line limit is 1 MiB. The JS helper allows `maxLineBytes` up to 8 MiB.
Closing input waits for the provider's return; cancelling output signals AbortSignal and waits for the sink to finish. The Node CLI also closes files and unfinished streams.
This example is a Node/JS capability binding; implementing WASI file/network packages remains future work.

<a id="タスクの取消し"></a>

### Task cancellation

```js
import {instantiate, TaskCancelledError} from './target/event-loop/index.js';
const {api, start} = await instantiate();
const controller = new AbortController();
const task = start('main', [60_000], {signal: controller.signal});
setTimeout(() => controller.abort('timeout'), 10);
try {
  console.log(await task.result);
} catch (error) {
  if (!(error instanceof TaskCancelledError)) throw error;
}
const outcome = await task.cancel(); // 要求を再送しても、同じ終了確認を返す
console.log(outcome);                // {status: 'cancelled', started: true/false}
console.log(await api.main(0));      // 取消し後も同じインスタンスで 42n
```

Names passed to `start` are JS export names (for example, `sumTicks` and `catchAll`). Pass arguments as an array.
`task.result` is a Promise returning the ordinary value; after cancellation, it yields TaskCancelledError with the first cancellation reason in cause. Cancellation is not converted into a Result err or a Wasm application exception.
`cancel(reason)` separates the request from completion confirmation. Its returned Promise remains unsettled until cleanup of the guest stack, owned readable ends, ABI buffers, and asynchronous host dispose/iterator.return completes.
Cancelling before start does not run the guest or transfer argument ownership. After start, cancellation is checked at suspension points and unwinds through an internal control exception. Source-level catch_all cannot catch it.
Completion reports status=returned/cancelled/failed and started indicating whether execution began. Later cancellation does not cancel tasks that already returned values or Future/Stream values already passed to callers.

Cancellation is cooperative. It does not forcibly stop CPU-only work, roll back changes already made, or stop language-side producer tasks.
If a custom host cannot finish an in-flight operation, completion confirmation keeps waiting. Failed cleanup returns failed and AggregateError and prevents reuse of the instance.
For direct `api` calls, async exports execute sequentially. Synchronous exports remain synchronous and return busy errors while another task executes or cleans up. Use `start` to queue synchronous exports too.

An idle synchronous `api` call keeps a cleanup record but does not create a public Task handle or result/completion Promises. Returned Future/thenable values remain values rather than being awaited internally. Asynchronous disposal still keeps the instance busy until cleanup finishes. `start()` always creates an observable queued task, including its cancellation acknowledgment and Result Promise.
The Component ABI and manifest use version=2. Regenerate old output with `just build-js`.
The Lean model verifies one-time receipt, delivery order, termination, and close, and is compared against actual Wasm execution.

See the [syntax and analysis boundary](syntax.md#標準-wat-との混在と解析) for Wasm exceptions versus traps and supported mixing of standard WAT and extensions.
`@try` expands to `try_table` and does not catch traps. See [exceptions.walz](../examples/exceptions.walz) for an executable example.

<a id="bytestext-とモジュールを-js-から使う"></a>

## Using bytes/text and modules from JS

```sh
just build-js tests/component-fixtures/buffers.walz target/buffers
```

```js
const { api } = await (await import('./target/buffers/index.js')).instantiate();
api.encode('日本語🦀');              // Uint8Array: UTF-8
api.decode(new Uint8Array([255]));   // undefined: option の none
api.text('hello');                  // 'hello'
api.slice(new Uint8Array([1, 2, 3]), 1, 2); // Uint8Array([2, 3])
```

Import selected standard names explicitly, as in `(@use "walz:core" (func $string.u64 $unsigned))`.
Standard library dependencies and all three applications use named imports too. Unused definitions and host imports are removed during generation, leaving only required capabilities in the manifest.
`@include` remains as compatible text expansion. Loading files does not add runtime I/O capabilities.

See [MEMORY.md](../MEMORY.md) for memory-management boundaries, corrected retention paths, and measurements. `just memory-check` rechecks management tables, GC reachability, fragmentation, and retained memory under repeated execution.

`walz/core/text.walz` provides concatenation, u64/s64 decimal conversion, and JSON string generation.
The [standard library](../walz/core/README.md) also provides search, splitting, checked integer parsing, and text/JSON builders.
Log aggregation, inventory ledger, and workflow use common parsing functions and the JSON object builder. `just test-stdlib` checks boundary values and both JS backends.
See [SPEC.md](../SPEC.md#文字列ライブラリと配列のコピー) for the new syntax and contracts.
`just fmt-walz file.walz` formats while preserving comments; `just check-walz-format file.walz` checks formatting.
`just compare-log-stats before-directory after-directory` alternates measurements under varied conditions for log aggregation before and after changes.

Shared memory can be passed to Core＋JSPI through `imports: { env: { memory } }`. It is retained separately from ABI temporary memory and shares the host's `WebAssembly.Memory` itself. See [type, ownership, Worker, and Component boundaries](threads.md).

To execute in another Worker, explicitly create a [generic WorkerHost](worker-host.md). A driver can instantiate another Core＋JSPI/Component program, await Futures, and return cloneable results. It provides per-task cancellation and join; CPU work needs cooperative cancellation checks. `@task.spawn` and GC references remain within the same agent. [Succinct indexes](succinct.md) also use the shared host.

### Experimental synchronous JS output

The [direct GC / JS string experiment](js-native-experiment.md) removes queued
Task/resource machinery only for an explicitly selected synchronous shared-value
profile. It keeps the stable Core＋JSPI and Component paths intact and measures
native strings separately from smaller generic glue.
