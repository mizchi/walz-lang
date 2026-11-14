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
only the documented subset. At that recorded revision it did not implement the
playground's display protocol; that capture does not describe the current
selectable self-hosted engine.

## Current selectable compiler

The current [Playground](playground.md#choosing-a-compiler) can load either the
Rust compiler or a verified self-built Core GC stage. Only the chosen engine
loads; the self-hosted mode provides expanded Walz and validation status rather
than native WAT and detailed source analysis. It remains a language subset.

The fresh `target/playground-selfhost/compiler-info.json` build records these
Wasm-body sizes, separately from the historical browser-request comparison:

| Compiler | Raw bytes | gzip level 9 | Brotli quality 11 |
| --- | ---: | ---: | ---: |
| Rust/Wasm | 8,840,457 | 2,111,760 | 1,347,821 |
| Self-built stage 2 | 489,204 | 77,975 | 59,514 |

These figures exclude JS glue, build metadata, the UI and HTTP overhead. The
compressed values are estimates; static hosting determines actual transfer
encoding. They are not speed measurements or full-feature parity claims.
The self-built artifact's source/seed hashes and stage equality are retained in
`selfhost/build.json` and its fresh build capture. Normal builds regenerate the
figures and both compiler artifacts; old benchmark captures are untouched.

Detailed per-file bytes, compression sizes, request lists, source/artifact hashes,
versions and parity results are saved with generated output modules. The complete
local capture retains the browser artifacts and native validation peer. Use a
fresh result path to remeasure that capture; use a fresh capture to compare a
changed compiler. Keep historical records unchanged.
