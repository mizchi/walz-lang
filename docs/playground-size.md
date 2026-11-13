# Browser compiler deployment size

The [recorded browser-size comparison](../benchmarks/selfhost/20261113-browser-size-v1-r1/README.md)
compares the existing Rust/wasm-pack playground compiler, a Rust WAT parser and
validator fixture, and the Walz-written compiler through Core and Component
value adapters. The measurement follows the captured unbundled deployment style.
This saved run predates the bootstrap compiler's GC type/checker/codegen support;
its sizes do not describe the expanded compiler. Keep the old capture unchanged
and make a fresh capture before comparing current deployment costs.

```sh
just test-playground-size
just build-playground-size target/playground-size/new-capture
just size-playground target/playground-size/new-capture target/playground-size/new-result
```

The capture and result directories must be new. The build uses the normal
playground builder with an isolated output path, both production selfhost
adapters and the locked reference in `tools/wat-web/`. It preserves the ordinary
`target/playground` directory and any server using it. `wasm-pack` 0.15.0,
Rust 1.99, Node.js 24+ and Playwright Chromium are used by the recorded run.

Each compiler runs in a fresh Chromium module-worker context against the same
eight-input integer WAT corpus as the [selfhost benchmark](selfhost-benchmark.md).
Before recording sizes, the tool validates all output, requires canonical module
bytes to match the native reference, checks 992 return/trap probes per compiler
and rejects an ill-typed input. Both Rust/Wasm variants also match native raw
output bytes.

The server records successful browser requests. Compiler payload totals include
only the requested compiler JS and Wasm resources, counted once. Type declarations,
package metadata, maps, unused files and the Component container used only as a
transpilation input are excluded. gzip level 9 and Brotli quality 11 are computed
independently per fetched file and summed. These are compressed-body estimates;
actual server encoding, HTTP headers and cache state are not measured. No further
JS minification, bundling or wasm-opt is applied.

The current Rust playground is also loaded normally, compiled and run. Its
page/CSS/editor/worker/sample files are measured separately from the compiler
payload. The common probe HTML and worker are excluded from compiler figures.

Feature scope matters: the Rust playground compiler includes standard-library
sources, macros, generics, analyses, WAT/diagnostic output and execution preparation.
The Rust fixture provides WAT assembly and validation without Walz extensions;
it still accepts broader WAT than the integer selfhost. The selfhost supports
only the documented subset and does not yet implement the existing playground's
WAT/analysis display protocol. The size comparison does not imply that a full
selfhost playground replacement is implemented.

Detailed per-file bytes, compression sizes, request lists, source/artifact hashes,
versions and parity results are saved with generated output modules. The complete
local capture retains the browser artifacts and native validation peer. Use a
fresh result path to remeasure that capture; use a fresh capture to compare a
changed compiler. Keep historical records unchanged.
