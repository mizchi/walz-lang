# Memory audit: runtime, recent lowering, and compiler

Measured on 2026-11-12 with Node v24.22.2 on macOS arm64 and Rust 1.99.0.
No persistent retention was detected in the workloads below. No production
runtime or compiler changes were needed; this run adds an audit harness and a
guest GC fixture.

The checkout includes uncommitted changes on top of
`7c1f463b6d059a627e913d206e9294eb5284349c`. The recorded source hashes, rather
than that commit alone, identify the measured implementation. Earlier dated
reports remain unchanged.

## Coverage and results

The recent suite runs five workloads on each of Core + JSPI, Component,
experimental native UTF-8, and experimental native JS-string output:

- Immutable range scans and nested byte views with 1–32 KiB inputs.
- UTF-8 views, validation, decoding, and materialization with Unicode and a BOM.
- Private tuple/Option/Result fusion, escaping values, handlers, and identity.
- Angle-bracket type applications and Result/tuple results.
- FlatBuffers encode/decode, nested records, Unicode, and malformed input.

Each workload runs 250 iterations per batch: one warmup batch and six measured
batches. Each iteration makes several guest calls. After each measured batch,
the harness advances the event loop and requests four GCs. Build tools run in
child processes, outside the sampled JS heap. Growth below is the difference
between the first and last measured samples within each case, not a comparison
between different bundles or backends.

| Output | Cases | JS heap growth per case | Linear capacity in every measured sample |
| --- | ---: | ---: | ---: |
| Core + JSPI | 5 | 28.7–72.1 KiB | 64 KiB |
| Component | 5 | 5.8–24.1 KiB | 64 KiB |
| Native UTF-8 | 5 | 9.7–57.8 KiB | 64 KiB |
| Native JS strings | 5 | 10.7–50.8 KiB | 64 KiB |
| Wasm compiler | 1 | 21.3 KiB | 2,240 KiB |

All 120 recent runtime samples have zero live arena allocations, host handle
entries, and inspected task/resource scopes. Native output has no boundary
buffer registrations and reuses a 64 KiB scratch buffer. Retaining a thrown
error after discarding each of the 20 instances does not retain its linear
memory. Every case has zero first-to-last ArrayBuffer growth.

The compiler is one long-lived wasm-bindgen instance. It compiles 672 inputs
(96 per batch), alternating optimized and unoptimized output, with distinct
function names and repeated syntax errors. After warmup its linear capacity
stays at 2,293,760 bytes, and ArrayBuffer growth is zero. This exercises the
browser compiler artifact under Node; it does not measure the browser UI heap.

## GC reachability and cleanup paths

The new raw-Wasm fixture tracks 389 guest objects. A mutable Wasm field, a live
view, and a live closure are positive controls: their referenced values must
survive GC while held. Clearing the field or dropping the view/closure permits
collection. Copying a view releases the original owner once the view is
dropped. Optimized helper and value frames also release their Node/bytes
objects. Every tracked object is eventually collected; the final batch needs
one observation pass. Each pass consists of four GC requests across job
boundaries, with at most eight passes allowed.

The unchanged core-expression suite was also rerun against this checkout:

- All 36 measured samples have zero live ABI allocations, delivery tables,
  task/resource scopes, and pending cleanup entries, at 64 KiB linear capacity.
- Each backend opens and closes 11,200 resources. All 448 selected closed
  resource objects per backend are collected while the instance stays live.
- A separate raw-Wasm test collects 448 guest GC objects across return,
  branch, Result sequencing, owned values, exception, and closure paths.
- Completed tasks, errors, and producer Promises can remain in history while
  the discarded instance's memory is collected after normal completion,
  cancellation, trap, destructor failure, and argument conversion failure.

The baseline suite reruns bytes/text ABI transfers, Future/Stream delivery and
consumption, and immediate closure of 60-second timer futures. It uses one
warmup and six measured batches: 1,000 iterations for buffers and delivery,
200 for immediate closure. Both backends finish naturally, with zero live
allocations or inspected table entries, stable 64 KiB linear memory, and less
than 2 MiB heap growth per case. Across these three stress suites there are
198 measured samples, including the six compiler samples.
The largest first-to-last JS heap increase across all runtime cases is
96,488 bytes (94.2 KiB); no case has first-to-last ArrayBuffer growth.

Existing tests run with `--expose-gc`: 48 runtime/retention/view/ABI tests,
13 Worker/channel/scoped-task tests, and 11 text/file I/O tests all pass with
no skips. They cover rejection and cancellation cleanup, held completed task
handles, Worker host collection, child-task histories, stream backpressure,
reader/provider collection, explicit file closure, and CLI process exit.
`just check`, `just test-docs` (four tests),
`node --check runtime/recent-memory-audit.mjs`, and the new just recipe's dry
run also pass.

## Reproduction and evidence

```sh
just memory-recent target/recent-memory-audit/recheck
just memory-core-expressions target/core-expressions-memory/recheck
just memory-check
just memory-core-js

node --expose-gc --test \
  runtime/workers.test.mjs runtime/channels.test.mjs runtime/scoped-tasks.test.mjs
node --expose-gc --test \
  runtime/text-io.test.mjs runtime/text-program.test.mjs runtime/node-io.test.mjs
```

The new suite requires the same Rust, Node, pnpm, and wasm-pack build tools as
the runtime and browser compiler. `--expose-gc` is mandatory. The processes do
not call `process.exit()` to hide live timers or Workers. Use a new output
directory for each measured run.

- [Recent suite report](recent-report.json) and [source manifest](sources.json).
- [Core-expression report](core-expressions-report.json), including its own
  source manifest and generated/instrumented artifact hashes.
- [Component baseline](component-baseline-report.json) and
  [Core baseline](core-baseline-report.json).
- [Additional source hashes](additional-sources.json) cover the compiler wrapper
  and build inputs outside the recent suite's source-manifest scope.
- [Runtime tests](runtime-tests.log), [concurrency tests](concurrency-tests.log),
  [I/O tests](io-tests.log), [documentation tests](docs-tests.log), and
  [static checks](check.log).
- [Recent measurements](recent-audit.log),
  [core-expression measurements](core-expressions-audit.log),
  [Component baseline log](component-baseline.log), and
  [Core baseline log](core-baseline.log).

The recent report is suite version 2. A pilot version 1 stopped because it
expected a string passed to an `s32` parameter to throw. Component canonical
ABI coercion accepts that value. Version 2 uses a Symbol for this negative
control, and null for the invalid bytes argument; the full suite was rerun in
`target/memory-20261112/recent-r2/`. This was an audit-input correction, not a
runtime leak or runtime fix. The core-expression and baseline workloads retain
their existing suite definitions. Reports here are copies of completed runs;
they must not be overwritten by future measurements.

## Limits

This is finite regression evidence for the current V8 engine and workloads,
not a proof that arbitrary guest code, engines, or external producers cannot
leak. The repeated-compiler test does not measure native Rust/LSP heaps or
browser UI retention. The 2 MiB growth threshold alone cannot rule out a small
leak; exact ownership counts and positive/negative reachability controls supply
additional evidence.

Returned live views and closures deliberately retain their owner/captures.
Callers must consume or explicitly close Future/Stream endpoints and release
external resources according to their ownership contract. A suspended guest
or a host that has not acknowledged cancellation may still own data. This
audit does not force disposal of objects that remain legitimately live.

Wasm linear memory retains its peak allocated capacity; released ABI buffers
are reused within it. V8 also retains caches and allocated pages, so RSS need
not fall immediately when values are collected. The samples record RSS for
inspection, but it is not used alone to classify a leak.
