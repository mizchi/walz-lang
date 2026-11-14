# Self-hosted compiler memory audit

The self-built Core/GC compiler has a repeatable retention audit in Node and
Chromium. It checks guest GC objects, copied output buffers, compiler lifetime,
and dedicated workers separately. It does not use the Core+JSPI or Component
ABI adapter: this compiler has no host imports or linear memory.

## Reproduce

Build the compiler and Playground first, then use fresh capture directories:

```sh
just playground-build
just build-playground-selfhost
just test-selfhost-memory
just memory-selfhost target/memory/run-1
just memory-selfhost-browser target/memory/browser-1
```

`build-playground-selfhost` prints a `target/playground-selfhost-build-*` capture.
Pass it as the third argument to include repeated compilation of the actual
compiler sources, with byte equality against the audited Wasm:

```sh
just memory-selfhost target/memory/self-build-1 target/playground/selfhost/compiler.wasm target/playground-selfhost-build-EXAMPLE
```

Each audit saves `report.json`, compiler bytes, source hashes, the recorder and
bridge source, and actual inputs. Existing captures are rejected rather than
overwritten. The browser audit starts a private HTTP server on an allocated
port and a headless Playwright browser; existing previews are untouched.
Its injected observer exists only in that audit server's worker response.

## What is checked

The workload mixes scalar operations, generic Option/match, 16 GC struct
shapes, parse errors, type errors, unsupported instructions, depth limits, and
16 KiB source comments. Identifiers change on every request. Two warmup batches
precede eight measured batches of 128 requests. Node runs this through both
the Core bridge and the Playground response adapter; Chromium uses the actual
deployed compiler worker and `WorkerClient`.

The observer keeps only weak references to real returned Wasm GC values. A
positive control holds two byte values solely through fields of a Wasm
source-file graph. They must survive GC while that graph is rooted, then be
collected after its root is removed. This checks the behavior of the observed
GC objects, including references from Wasm fields.

At each checkpoint, forced GC follows a completed request and event-loop turn.
No observed request value may remain alive. Retained heap growth is the mean
of the final two samples minus the mean of the first two. Limits are 2 MiB for
heap/external/embedder memory and 1 MiB for ArrayBuffer/backing storage.
Synthetic growing heaps must fail this detector; missing heap metrics also fail.
RSS is recorded separately. It is not a substitute for retained live memory.

The optional self-build phase compiles the captured 45-file compiler project
five times: one warmup and four measured calls. Each output must match the
audited compiler SHA-256. Its GC checkpoints use the same retention criteria.

Browser lifecycle checks require:

- Discarded response buffers become unreachable while one compiler stays active.
- Cancelling the client leaves no compiler worker targets or Worker wrappers.
- Six cancellations while loading retire all worker targets and wrappers.
- Twelve Rust/self-hosted switches and runs retain one current compiler worker
  and one current output buffer. Retired workers and results are collected.
- A larger synchronous compilation receives Stop 20 ms after its call-entry
  signal, before a return signal. Its worker target and wrapper disappear.

Node also retains a host argument error and an injected real Wasm trap while
dropping the compiler API. The errors must not keep the compiler's exported
functions or request values alive. This covers exception cleanup independently
of the worker's normal error serialization.

Metrics use Node's process/V8 APIs and Chromium's
[Runtime.getHeapUsage](https://chromedevtools.github.io/devtools-protocol/tot/Runtime/#method-getHeapUsage)
and [HeapProfiler.collectGarbage](https://chromedevtools.github.io/devtools-protocol/tot/HeapProfiler/#method-collectGarbage).
These are engine measurements, not portable language guarantees.

## Findings and limits

The [2026-11-15 capture](memory/20261115-selfhost-r1/README.md) records the exact
artifact, inputs and samples: 3,840 mixed requests, five byte-identical
self-builds, zero live observed request values, and 13.7–32.4 KiB of retained
heap growth across the measured phases. Worker retirement and held public
exceptions also passed. RSS remained much larger than live heap.

The audit reproduced a retention path in the JS stage bridge: a saved Error's
lazy V8 stack kept a bridge closure and its Wasm compiler instance alive.
The public `imports`, `compileProject`, and `expandProject` boundaries now
materialize error stacks before rethrowing. Error identity, messages, causes,
and AggregateError members are preserved; cyclic causes are handled. The
regression test retains errors across GC, including nested causes and an actual
Wasm trap, rather than releasing the errors before checking collection.

Bootstrap bytes use a persistent tree of i32 values, not a packed byte array.
Holding only 32 KiB of logical source data increased retained heap by several
MiB in the positive control. Releasing the graph recovered that heap, while
RSS could remain high. Repeated compilation also left a much larger resident
capacity than live heap. Packed GC byte storage and lower temporary allocation
are separate performance work; collection alone does not establish a low
memory footprint.

This is a finite regression check for the recorded inputs, artifact, Node/V8,
and Chromium versions. It does not prove absence of every leak, bound peak
memory, or audit other engines. The observer sees boundary objects rather than
every internal allocation; aggregate heap/backing-store trends provide an
additional check. The binary's table/memory/global section counts are recorded,
including unexported roots. Engine caches and intentionally retained current
UI results have different lifetimes from per-request source and IR graphs.

The ordinary Core+JSPI and Component adapters have their own
[memory checks](../MEMORY.md); this audit does not replace them.
