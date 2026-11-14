<a id="ブラウザーのプレイグラウンド"></a>

# Browser playground

Offers the full Rust/Wasm compiler and a smaller compiler self-built in Walz for
editing, compilation and execution in the browser. The Compiler menu selects
one engine; only the selected compiler's resources load. Use `?compiler=selfhost`
to open directly in the self-hosted mode without downloading Rust/Wasm.
Sources are not sent to a server for compilation. No JavaScript bundler or server-side Rust execution is needed.

[Open the published version](https://mizchi.github.io/walz-lang/). Documentation on the page links to the README and language-feature guides.

<a id="起動とビルド"></a>

## Starting and building

Use Rust 1.99 or later, Node.js 24 or later, pnpm, just, wasm-pack, and a browser supporting Wasm GC.

```sh
cargo install wasm-pack --locked
rustup target add wasm32-unknown-unknown
pnpm install --frozen-lockfile
just playground
# http://127.0.0.1:41740
```

For a build only, use `just playground-build` (or `pnpm playground:build`).
This generates HTML, CSS, ES modules, `pkg/walz_web_bg.wasm`,
`selfhost/compiler.wasm` and build/size metadata under `target/playground/`.
The entire directory can be deployed to any static HTTP server; relative URLs support subdirectory hosting.
Serve `.wasm` as `application/wasm`, and `.mjs`/`.js` with a JavaScript MIME type. Open over HTTP rather than `file://`.
Serve local built artifacts with `pnpm playground`.

wasm-pack uses `--target web --release`. The first run may take longer because it also downloads the matching wasm-bindgen tools.
wasm-opt is disabled for this crate to avoid an additional Binaryen dependency. Rust release optimizations remain enabled.
Serve generated JS and Wasm from the same build together.
For option details, see the [wasm-pack build documentation](https://rustwasm.github.io/docs/wasm-pack/commands/build.html).

The self-hosted build first creates a Rust-built seed, then compiles the Core GC
bridge as stage 1 and again with stage 1 as stage 2. Publication requires
byte-identical, independently validated stages with no host imports. Stage 2 is
the deployed compiler. Fresh build captures retain seed hashes, source snapshots
and both stages under `target/playground-selfhost-build-*`. Build only this
artifact with `just build-playground-selfhost`.

## Choosing a compiler

| Feature | Rust | Self-hosted subset |
| --- | --- | --- |
| Language | Full browser compiler | Bounded i32/i64, GC, selected core collections, generics and expression macros |
| Source view | Generated native WAT | Expanded Walz; some high-level forms remain |
| Analysis | Detailed source analysis | Supported-subset validation status |
| Optimization control | On/off | Unavailable |
| Async event-loop execution | Core/JSPI | Unsupported |
| Execution/download | Core Wasm | Core Wasm |

Switching compilers preserves editor text, cancels the current compiler worker
and clears executable results. Unsupported self-hosted forms produce
`E_SELFHOST_<number>` with original UTF-8 ranges and line/Unicode-column locations.
No automatic Rust fallback occurs on unsupported syntax or load failures.
Normal source errors keep the selected compiler usable; failed loads can be retried.
The self-hosted compiler itself needs browser Wasm GC support, but not JSPI.
Rust remains the default and the language-tour expectations describe that mode.

The current Playground self-hosted Wasm is 489,204 bytes (about 478 KiB), or
77,975 bytes with gzip level 9 and 59,514 bytes with Brotli quality 11. These
figures cover the Wasm body, excluding JS, build metadata and the shared UI.
The same build's Rust/Wasm body is 8,840,457 bytes. Compressed figures are local
estimates, not a measurement of server encoding or HTTP overhead. Features differ;
the size difference is not a full-feature equivalence or speed comparison.
Each build writes its current figures to `compiler-info.json`.

<a id="github-pagesへの公開"></a>

## Publishing to GitHub Pages

The [Pages workflow](../.github/workflows/pages.yml) builds the compiler from the same commit to Wasm on pushes to `main`, checks samples, the Rust adapter, and Chromium, then publishes `target/playground/`. Pull requests build and check without publishing. It can also be run manually from Actions.

Set the publishing source to GitHub Actions under the repository's Settings → Pages. Generated artifacts need not be committed to a source branch. Page samples and descriptions are in English; detailed language documentation is in repository Markdown.

<a id="対応範囲"></a>

## Rust compiler scope

- Compilation of single-file Walz, standard WAT, and bundled `walz:core`.
- Type-argument inference for ordinary generic calls, hygienic user macros, public type-information queries, and explicit binding scopes.
- Name, type, effect, and ownership checks, optimization/DCE, and the Wasm validator shared with the CLI.
- Generated WAT, analysis JSON, and diagnostics including error codes, positions, related positions, notes, and help.
- Optimization toggling and Core Wasm downloads.
- Execution of zero-argument function exports in Core Wasm. `i64` displays as `42n`, multiple results as arrays, and no result as `unit`.
- `sleep`, `yield`, `defer-i64`, `ticks`, and `fallible-i64` from `walz:host/event-loop@0.1.0`, executed in browsers supporting Wasm GC and WebAssembly JSPI.

GC references remain Wasm references; their contents are not expanded from JavaScript.
This page does not provide file imports/includes, host I/O such as files/networking, or execution of Components themselves.
Modules with host imports beyond the event loop can still be compiled and saved; execution reports missing import names.
Exports requiring arguments produce execution diagnostics. Additional Core＋JSPI/Component host integration is available through the CLI.
When the browser lacks a Wasm feature accepted by the compiler, WAT display and downloads remain available, and the reason execution is unavailable is shown.

`@law` and verified compilation requiring an external Lean kernel are unavailable in the browser.
Generation does not skip verification; it returns `E_HOST_UNAVAILABLE`.

<a id="言語機能のサンプル"></a>

## Language feature samples

The Example menu offers 44 samples. Descriptions and expected results are in English, matching the interface.

| Group | Count | Features introduced |
| --- | ---: | --- |
| Basics | 3 | Multiple bindings, typed literals, operators, short-circuit logic, bit operations, floating point, and explicit conversions |
| Control flow | 5 | if, block, return, br, state-passing loop, for, mutual recursion, and Wasm exceptions |
| Data and types | 8 | Named structs, GC arrays, enums and exhaustive match, Result and do notation, Option, tuple/field destructuring, for-each, field updates, aliases, and newtypes |
| Functions and modules | 5 | Explicit captures, type-argument inference and monomorphization, type-class constraints, file modules, @use of WIT-bearing Components, opaque types, owned values, and one-shot closures |
| Syntax extensions | 6 | Hygienic macros, use of inferred types, repeated bindings and delegation, public selectors, and diagnostics for initializers, repetition positions, and body forwarding |
| Core library | 7 | Immutable list map/fold, callable constraints, string/bytes, zero-copy string/bytes/array-view, dict, JSON, and v128/SIMD |
| Ownership and async | 4 | Already-delivered Futures and finite Streams, fold-result, endpoint close, host suspension boundaries, resources, and borrows |
| Checks and interop | 6 | test, bench, type/move/effect diagnostics, law verification boundaries, standard WAT, and multiple results |

34 samples can run directly with Run, comparing `Expected output` with actual results.
Host Future examples also await Promises through JSPI within the browser.
The remaining 10 show `Expected diagnostic`. Alongside type, ownership, and effect errors and macro scope/repetition constraints, they demonstrate browser restrictions:
modules requiring file resolution, trusted `--bindings` settings required for resources, and the Lean kernel required for laws.
The resource example shows that declaring a host import alone does not establish permission; no provisional permission is granted merely to pass checking.

Already-delivered Future/Stream examples read locally constructed values without starting language-side producers or an event loop.
Close examples likewise abandon already-delivered endpoints, distinct from cancelling pending host tasks.
The single-file page does not resolve file @use/include. The module example shows two files and CLI usage, expecting `E_SOURCE_REQUIRED` on the page.
The WIT-bearing `.wasm` example is also for the CLI; uploading or executing Components through the page is unsupported. [Binary constructors](wasm-use.md) describes explicit instantiation, native async ports, and instance/store lifetime.

Editing input clears the sample's expected result and switches to `Custom source`.
Selecting another sample restores its description, expected result, and source together.

<a id="処理の分離"></a>

## Separation of processing

`crates/walz-web` is a thin wasm-bindgen adapter calling the existing `walz::compile_with_options`.
`compile_source(source, optimize)` returns a JSON string in `walz-playground-v1` format.
Success returns `status: "success"`, `wat`, `wasm` (a byte array), and `analysis`; failure returns `status: "error"` and the existing `diagnostic`.

The self-hosted adapter uses the same browser envelope with `sourceView:
{format: "walz", text}` instead of a native `wat` string, and explicitly marks
analysis as `validation-only`. The Core GC bridge constructs/accesses opaque
guest values; JavaScript transfers files and bytes and formats diagnostics.
It does not parse, resolve, type-check or generate executable source on behalf
of the self-hosted compiler. Its hash is checked against the fixed-point build
metadata before loading. This bridge is not a generated Canonical ABI adapter.

For event-loop imports, `prepare_execution(source, optimize)` generates the existing Core＋JSPI adapter in memory.
Its response uses `walz-playground-execution-v1`; on success, `program` contains Core Wasm, ABI-memory Wasm, generated bindings, the manifest, and export names and argument counts.
The execution Worker uses the existing `runtime/core.mjs`, `host.mjs`, `tasks.mjs`, and `resources.mjs` to connect Promises and AsyncIterables.
Generated bindings load as Blob ES modules and release their URLs, so a deployment CSP must allow `blob:` module imports inside Workers.
Saved Core Wasm is unchanged; the downloaded file alone does not include a JavaScript host or adapter.

The browser adapter checks Rust type, effect, and ownership rules, trusted imports, WIT boundaries, and export contracts.
It does not claim independent verification by an external Lean kernel and rejects inputs containing laws. CLI verified compilation continues to invoke Lean.

Interface and runtime messages are in English.
The selected compiler lives in a reused Web Worker. Switching destroys the old
worker and compiler instance; the Rust engine also reuses its standard-library syntax cache.
Generated Wasm execution gets a fresh Worker and instance each time. Infinite loops, including start functions, do not block the main interface.
Workers terminate after 30 seconds for compilation and 2 seconds for execution. Stop also terminates them, and the next operation recreates them.
This stops CPU processing; it does not guarantee a memory-usage limit for the whole browser process.
Changing the source or optimization settings invalidates artifacts; stale compilation responses and execution results are not displayed.

<a id="検査"></a>

## Checks

```sh
just test-playground
```

Rust adapter tests compare native WAT, GC extensions, optimized/unoptimized output, and diagnostics against the existing API.
Comparisons with native Core＋JSPI generation run the ownership kernel, so the checking environment also needs `lean +v4.34.1`. Pages CI prepares pinned tools through the existing `ci/install-tools.py`. Running the published Wasm compiler does not require Lean.
`playground/samples.test.mjs` checks feature coverage and description/expectation metadata.
`playground/tests/samples.spec.mjs` compiles every sample in the browser, comparing execution values, missing host imports, and intended diagnostics.
Playwright checks Wasm compiler loading, GC samples, execution/saving, diagnostics and recovery,
infinite-loop timeout/stop, Lean rejection, traps, missing host imports, and narrow screens.
It also checks event-loop Future/Stream, optimization off, Result failures, export-name conversion, argument counts, and rerunning after stopping a wait.
Self-hosted checks cover lazy loading without Rust, GC/generic execution,
expanded-source labeling, errors/recovery, download, cancellation during engine
loading, load failure/retry and narrow layout. Pure response-adapter tests also
check Unicode locations and absence of executable bytes on errors.
For seven fixed inputs, it additionally checks that the Wasm compiler's WAT matches the recorded pre-change native output.
After building, `pnpm test:playground` runs browser checks alone.
If another local server uses the default port, change the test port with `WALZ_PLAYGROUND_TEST_PORT=41741 pnpm test:playground`.
Set `WALZ_PLAYGROUND_TEST_ROOT` to test an isolated build directory.

Sample definitions are split by feature under `playground/samples/` and collected in `samples.mjs`.
Add samples with `category`, `label`, `description`, `features`, `expectation`, and a self-contained `source`.
Set `expectation.kind` to `run` (`output`), `compile` (host `imports` and explanatory `note`),
or `diagnostic` (`code` and optional `message`), and verify actual behavior with `just test-playground`.

Sources containing scoped child tasks execute using Core＋JSPI output and a dedicated scheduler. WAT display includes the actual private ABI in that case, while the function selector shows only exports from the original source. See the [spawn / join contract](scoped-tasks.md).
