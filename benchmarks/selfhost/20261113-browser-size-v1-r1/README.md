# Browser compiler deployment size v1

Recorded 2026-11-13T11:10:37.482Z. Apple M5, darwin/arm64, Node v24.22.2, Chromium 156.0.8078.4, rustc 1.99.0 (b940084d7 2026-09-28), wasm-pack 0.15.0.

## Compiler payload

Measured from successful HTTP requests made by a fresh Chromium module worker that compiles and runs the frozen eight-input scalar corpus. Each compiler uses the same probe. Sizes are KiB (1 KiB = 1024 bytes). gzip level 9 and Brotli quality 11 are computed separately for each fetched resource and summed. They model compressed response bodies, not observed Content-Encoding or HTTP header bytes.

| Compiler | Files | Wasm raw | JS raw | Total raw | gzip | Brotli |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Rust full playground compiler | 2 | 6255.75 | 7.82 | 6263.57 | 1524.17 | 997.44 |
| Rust WAT parser + validator reference | 2 | 2649.17 | 7.21 | 2656.39 | 565.25 | 388.51 |
| Selfhost Core + value ABI | 8 | 51.27 | 33.45 | 84.72 | 28.79 | 24.49 |
| Selfhost Component + value ABI | 7 | 54.33 | 154.74 | 209.07 | 54.97 | 46.29 |

Core uses 2.5% of the full Rust compiler's Brotli payload (40.7 times smaller), and 6.3% of the WAT/validator reference (15.9 times smaller). The Component adapter is 1.89 times Core's compressed size. This is a feature-sensitive comparison, not equivalent implementations of the whole language.

Rust full includes Walz's standard-library source catalogue, macros, generics, static analyses, WAT/diagnostic output and execution preparation. The Rust WAT reference has only assembly/validation and a thin wasm-bindgen API, but accepts broader standard WAT than the scalar selfhost. The selfhost offers compile/check for its integer subset and does not provide the existing WAT/analysis UI protocol or the full library/language.

## Existing playground page

The real Rust playground was also loaded in a fresh browser context, its default source compiled and its first Run completed with output "42". These totals include page, CSS, editor/worker code, example modules and the Rust compiler. The shell row excludes compiler pkg assets; it is not a measured full selfhost playground replacement.

| Scope | Raw KiB | gzip KiB | Brotli KiB |
| --- | ---: | ---: | ---: |
| Initial default compile | 6314.25 | 1541.26 | 1011.74 |
| Through first Run | 6348.34 | 1552.18 | 1021.34 |
| UI/worker/sample files through first Run | 84.77 | 28.01 | 23.90 |

Compiler-only figures exclude the shared measurement HTML/worker, program output binaries and the real playground shell. A future selfhost playground still needs UI/worker adaptation and shell files. It cannot currently replace the full-featured playground.

## Validation and accounting

All four browser compiler paths passed full Wasm validation, exact canonical-module equality with the native reference, 992 return-value/trap probes each and rejection of ill-typed input. Both Rust/Wasm paths also matched native raw module bytes. Canonicalization preserves signatures, function/local/export order and every instruction/immediate. See [the parity contract](../../../docs/selfhost-benchmark.md).

Only browser-requested compiler assets are counted, with duplicate URL/query variants counted once. Type declarations, package metadata, source maps, old-name leftovers and unused standalone Component containers are excluded. The Component measurement includes its generated core Wasm modules and JS glue. This follows the current unbundled playground deployment style; neither JS minification nor additional wasm-opt was applied. Both wasm-pack packages explicitly disable wasm-opt, and use target web/release with locked dependencies.

A clean isolated build avoids overwriting the running development playground and avoids stale files in an existing pkg directory. The normal build tool now accepts an optional output directory while keeping its default. Its [recorded build helper](build-playground.mjs) is retained; [results.json](results.json) separates build-source hashes from measurement-tool hashes.

## Reproduce

From the repository root with the pinned Rust/Wasm toolchain, Node.js 24+ and Playwright Chromium installed:

```sh
just test-playground-size
just build-playground-size target/playground-size/new-capture
just size-playground target/playground-size/new-capture target/playground-size/new-result
```

Both capture and result paths must be new. The complete local browser artifacts are retained at /Users/mz/ghq/github.com/mizchi/wlt-lang/target/playground-size/capture-v1. [capture.json](capture.json) records their file hashes and source provenance; [results.json](results.json) records exact byte counts, request paths, compiler assets and validation. Browser-generated modules are in [outputs](outputs/) and frozen source/probe/reference modules in [inputs](inputs/). The [Rust WAT fixture](wat-reference.rs) and code snapshots are retained for provenance. Snapshots are not a standalone deployment package.

The first attempt completed compiler measurements but failed resolving the playground directory URL to index.html; it was not published as a successful run. Its failure record is retained locally at target/playground-size-dev/failed-first-attempt. The directory-path regression is covered by a test, and this r1 record is a fresh successful measurement.
