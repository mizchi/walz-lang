# Validation record

Passing final checks:

- `stream-tests.log`: 13 raw facade, typed input, row-property, snapshot,
  getter reentrancy, Cell identity, transfer-bounds, exception cleanup and GC tests.
- `stream-backends.log`: 5 Core/Component pipeline, TypeScript and expression tests.
- `stream-e2e.log`: 3 Chromium checks; raw facade plus Core/Component snapshots.
- `compiler-final.log`: 58 Rust library/region/readonly/optimization checks,
  including differential region-summary equivalence tests.
- `native-final.log`: the native CLI integration test, asserting four successful
  Walz test cases on Wasmtime. It takes 200 seconds in the release profile.
- `rust-validation.log`: the raw checked-IR/SIMD and opaque/effect contract tests
  passed. This exploratory run's native subtest was intentionally stopped while
  tuning region analysis, so the file ends with a failure. `native-final.log` is
  the completed successful replacement, not a suppressed failure.
- `cancellation-final.log`: Core and Component cancellation releases siblings,
  captures and ABI allocations. Raw numeric transfers are synchronous and have
  no cancellation/suspension point within a borrow.
- `check-last.log`, `docs-last.log`: stdlib generated docs, Rust formatting,
  Clippy with warnings denied, and three documentation/workflow checks.

The Red logs and interrupted-build samples are exploration records. Earlier
missing-artifact and browser-server path/existing-output errors were harness
setup failures, followed by the passing final runs above.

The browser check used the table-only `browser-server.mjs` and Playwright config
retained here, with fresh Core/Component builds and the exact checked raw Wasm.
It avoided rebuilding 111 unrelated browser fixtures. To reproduce, copy these
two helpers into `target/table-io/`, copy the captured `core.wasm` into
`target/table-io/primary/`, then run Playwright with that config from the repo
root. The temporary browser output directory must be absent before each run.
The regular `runtime/e2e/table.spec.mjs` remains part of the normal browser suite.

No build, test or profiler ran during the recorded paired timing samples.
All three candidate kernels are byte-identical, including after the exact
region-analysis implementation changes. The capture helpers record the compiler
profile/path; the frozen source and Wasm hashes are in each candidate manifest.
