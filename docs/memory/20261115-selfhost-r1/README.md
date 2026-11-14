# Self-built compiler retention audit

Measured on 2026-11-15 JST, macOS arm64, Node v24.22.2 / V8
13.6.233.17-node.63 and Chromium 156.0.8078.4.

The 489,204-byte self-built compiler has SHA-256
`a84ee551f5722be9aa7e33b0f865ec5b3b61cd995ee00058749e76b78f9a7f93`.
It has no host imports, tables, linear memories or globals, including unexported
ones. The language/compiler sources and binary are identified in
[compiler-build.json](compiler-build.json); the repeated self-build inputs are
identified in [selfbuild-inputs.json](selfbuild-inputs.json).

One retention path was found and fixed: saving a public stage-bridge exception
could retain the compiler instance through V8's lazy stack. Stack materialization
at the bridge boundary now releases that reference while preserving error
identity and nested causes. The [Red log](bridge-red.log) reproduces retention
with a held host-argument error. The [Green log](bridge-green.log) covers host
argument errors, actual Wasm trap unwinding, imports, expansion, and nested
AggregateError/cyclic causes while errors remain in history.

## Results

Each mixed phase runs 1,280 requests: two warmup batches and eight measured
batches of 128. The two Node phases and one browser phase therefore exercise
3,840 requests. Four success families and four rejection families use distinct
identifiers and 16 physical GC struct shapes.

| Scope and phase | Retained heap growth | Backing-store growth | Live observed request GC values |
| --- | ---: | ---: | ---: |
| Node process, Core bridge | 13.7 KiB | 0 B | 0 at every checkpoint |
| Node process, Playground response adapter | 30.1 KiB | 0 B | 0 at every checkpoint |
| Node process, actual self-build | 32.4 KiB | 0 B | 0 at every checkpoint |
| Chromium compiler worker | 21.1 KiB | 0 B | 0 at every checkpoint |

Growth compares the final two post-warmup sample means with the initial two.
This is not a comparison of absolute engine footprints: Node includes the
recorder and prepared inputs, while Chromium samples the compiler worker.
Node's retained heap stayed around 7.8–8.1 MB; the worker stayed around 0.75 MB.
All fixed criteria passed. Raw samples are in [node-report.json](node-report.json)
and [chromium-report.json](chromium-report.json).

The real compiler project has 45 source files totaling 838,608 bytes. Five
self-builds, one warmup and four measured, each emitted the exact compiler
SHA-256 above. All measured backing-store sizes were constant within each phase.
The later phase deliberately retains its larger source fixtures; its absolute
buffer capacity is not compared with a phase that has fewer inputs.

The Wasm-field positive control retained both real byte GC objects while rooted
and collected both after dropping the graph. Node retained an argument error
and injected real Wasm trap after dropping its compiler API: compiler functions
alive = 0, request GC values alive = 0. Native contexts stayed at 1 and detached
contexts at 0 during all measured phases.

Chromium also passed:

- Reusing one worker while all discarded response buffers were collected.
- Six cancellations during compiler download: 0 retired Worker wrappers and
  0 worker targets after each cancellation.
- Twelve Rust/self-hosted UI switches and executions: one active compiler
  worker and one current output buffer, with prior wrappers/results collected.
- Stop during a larger synchronous compile, 20 ms after its call-entry signal
  and before its return signal: 0 worker targets, wrappers and output buffers.

The injected trap calls an actual Wasm byte accessor out of bounds inside the
public compilation boundary. It tests unwinding/cleanup, not a fabricated JS
exception. Worker cancellation uses the existing Stop handler, with no busy
loop or artificial delay inside the guest.

## Memory footprint remains work

The largest sampled Node RSS after GC was 1,019,248,640 B, about 972 MiB, despite
roughly 8 MB of live heap. It declined to about 716 MiB after teardown. These
are sampled resident sizes, not peak-allocation measurements or a memory bound.

Holding 32 KiB of logical source bytes increased Node retained heap by about
2.65 MB and Chromium worker heap by about 1.45 MB. The bootstrap implementation
represents bytes as a persistent tree with i32 leaves and intermediate builder
nodes. Removing the graph's root released its observed byte objects and live
heap. Packed byte storage and temporary allocation reduction remain on TODO.

The observed resident capacity is substantial, even though these finite runs
show no persistent request retention after the exception fix. Do not interpret
the pass as a proof for every program, engine, internal allocation, or peak size.

## Validation and provenance

- [69 Playground E2E cases](playground-e2e.log).
- [Four Core/Component parser, checker, codegen and cleanup cases](adapters.log).
- [Retention detector and held-error regression tests](bridge-green.log).
- [`just check test-docs test-skill`](checks.log).
- [Documentation checks after adding this record](docs-final.log).
- Complete [Node log](node.log) and [Chromium log](chromium.log).

The checkout contains uncommitted work on the base commit in
[capture.json](capture.json). Reports contain hashes for the actual recorder,
bridge and deployed worker sources; full source/input/compiler copies remain
in the fresh `target/selfhost-memory/node-4` and `browser-7` captures. Earlier
failed/exploratory captures remain separate. The private Playground copy uses
the tested bridge; existing preview files and servers were left untouched.
The response adapter's final blank line was removed before commit; its
[formatting record](source-formatting.json) retains both hashes. Measured source
copies remain verbatim, and executable code is identical.

See [the audit guide](../../selfhost-memory.md) for commands, controls,
thresholds and scope. Other runtime ABI checks remain separate.
