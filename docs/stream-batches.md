# Stream Batch Transfer

```wat
(@match (@stream.read-batch i32 256 (@move $stream))
  (case pair ($next $items)
    (block
      ;; $items は不変 list<i32>。$next は引き続き一つの所有値。
      (@stream.close i32 (@move $next)))))
```

`@stream.read-batch T max stream` consumes an owned `stream<T>` and
returns `tuple<stream<T>, list<T>>`. `max` is an i32 from 1–65536. An empty result list means EOF.
The host can return partial progress of one through max elements without waiting for the requested count.
Existing `@stream.next` still reads one element. In either case, consume or close the returned tail.

Only shareable payloads are allowed, preserving element order, GC value types, and UTF-8 checks.
Standalone Core ready streams copy from GC cons chains.
Core+JSPI and Component receive a Canonical buffer in one read and transfer it into a GC list.
In addition to capacity limits, the Canonical buffer is restricted to the existing 16 MiB limit.
Cancellation prevents delayed producers from writing released buffers and waits for reader shutdown.

## Host Protocol

An optimized AsyncIterator can implement `readMany(max)`, returning
`Promise<{items: Array | TypedArray, done: boolean}>`.
`items` contains 0–max elements; an empty result requires done=true.
The final elements may be returned together with done=true.
Ordinary AsyncIterators read one element at a time through next().

Readers from `createChannels()` and file adapters support this protocol.
Pinned Component jco glue uses an internal batch marker to distinguish batches even when a tuple/list is itself the payload.
A change in the generated format causes build rejection.

`createChannels().sendMany(writer, values)` accepts an array of at most 65,536 elements and
returns `Promise<[accepted, status]>`. Status is 0=all accepted, 1=closed/cancelled.
Accepted is the length of the prefix passed to a buffer or reader; cancellation does not roll it back.
Unaccepted elements are released. An empty array succeeds with zero progress and does not wake pending readers.
Existing rules of one pending read/write and capacity-0 rendezvous remain.
Walz import policy uses `(list T) -> (pair i32 i32)`, explicitly specifying resource use and suspends.

## Tests and Measurements

`just test-runtime-cli` checks ready streams, strings, unit, ordering/partial acceptance/cancellation in both JS backends,
and zero outstanding allocations. Reading 511 elements with a maximum of 13 calls the producer 40 times.
This checks host round-trip count, not the speedup itself.

`sum-batch(size)` in the [file example](../examples/wasi-files/README.md)
groups a byte stream into GC lists and sums them with `list.fold`.
After generating WIT bindings, `just bench-stream-file /absolute/path/input.bin` compares
1-byte and 256-byte batches using the same input and guest processing.
Measurement includes file open/close, I/O, ABI conversion, JSPI, and GC work.

Ready-only ordering corresponds to existing `WalzDelivery.readMany_preserves_order`.
There is no machine proof of host asynchronous implementation/glue/scheduler semantics preservation or liveness.
EOF and successful file completion futures are distinct; check completion even after reading ends.
