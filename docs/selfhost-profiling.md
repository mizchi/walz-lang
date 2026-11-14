# Bootstrap frontend profiling

For declaration/argument attribution and actual Wasm body sizes, use the separate
[specialization inventory](selfhost-sharing.md). Counter totals alone do not
measure redundant generated code.

The Walz compiler can report expansion work and cache reuse for one project,
including a rejected project. The guest performs linking, normalization and
independent validation. JavaScript supplies files and records the returned data;
it does not implement a compiler fallback.

## Run a profile

```sh
just build-selfhost-profile
just selfhost-profile examples/selfhost/project/main.walz target/profile/project.json
just selfhost-profile examples/selfhost/portable.walz target/profile/compiler.json
```

The compiler-source probe now completes within the supported self-hosting
subset. Rejected projects also write a JSON report with their source diagnostic
and partial counters. The optional final argument selects another profiler artifact:

```sh
just selfhost-profile examples/selfhost/project/main.walz target/profile/project.json target/profiler-before
```

The separate entry, `examples/selfhost/profile.walz`, exports `profileProject`,
`checkProject`, `compileProject` and `imports` through the ordinary Core+JSPI or
Component adapter. The production portable compiler retains its five operations.

`profileProject(files)` returns `{result, stats}`. `result` has the same checked
function count or original `{file, code, start, end}` diagnostic as
`checkProject(files)`. A failed expansion returns its partial statistics. The
CLI performs guest preflight before dependency I/O; a failure during discovery
has `stats: null` and phase `discovery`.

## Counters and cache contracts

A concrete instance is keyed by declaration identity and normalized type
arguments. Nominal types remain distinct; imported aliases keep the identity and
opaque-access rules of their original declarations.

Each function instance now caches its immutable resolved parameter/result types
and its return type. Results are published only after successful resolution.
Concrete heap instances also cache substituted layouts and resolved field types.
Source access checks still run at each operation. Return types can be requested
before parameters, retaining the previous
resolution order. Function emission also reuses those parameter/result types.
This is not a structural equivalence cache over arbitrary syntax or nominal
layouts. The existing alias/instance caches remain in use.

| Statistics | Meaning |
| --- | --- |
| `signatureRequests`, `signatureHits`, `signatureComputations` | Parameter/result signature requests, cache hits and attempted misses. |
| `returnRequests`, `returnHits`, `returnComputations` | Return-type requests, hits and attempted misses. Signature construction can request the return type. |
| `instanceRequests`, `instanceHits`, `instanceComputations` | Concrete-instance lookups, hits and attempted misses. Rejected misses need not create an instance. |
| `aliasRequests`, `aliasHits`, `aliasComputations` | Existing alias normalization cache use. |
| `heapInstances`, `functionInstances`, `aliasInstances` | Successfully created cache entries by kind. |
| `layoutRequests`, `layoutHits`, `layoutComputations` | Concrete composite-shape lookups and attempted cache misses. |
| `fieldRequests`, `fieldHits`, `fieldComputations` | Resolved field/storage-type list lookups and attempted cache misses. |
| `typeVisits` | Attempts to enter type resolution, including early returns and depth failures. |
| `typeWork`, `expressionWork`, `logicalWork`, `namedFieldWork` | Charged work, split by operation. Their sum is the elaboration budget consumption, including the visit crossing a limit. |
| `files` | The same four work counters by original source file. `-1` denotes embedded core; other indices address the supplied file list. |
| `maxDepth` | Maximum attempted shared type/expression traversal depth, with zero at the outer entry. It is not native stack, parser or lowering depth. |
| `renderedBytes` | In v2, bytes written during folded-source rendering, including a failed partial rendering. |
| `syntaxNodes`, `syntaxStorage` | In v3, directly emitted syntax nodes and logical storage (decoded values plus four bytes per child index), including partial construction on failure. These are not GC heap measurements. |
| `syntaxVisits`, `syntaxAtomHits` | In v4, logical tree visits and same-origin short-atom hits for the main arena. Hits still consume the visit budget. On complete conversion, nodes plus hits equal visits. |

Counters and caches belong to a single compilation request. Ordinary entry
points pass no profiling state and allocate no counter arrays. Profiling returns
an immutable guest snapshot; it retains neither the compilation state nor the
mutable counter arrays. Node and Chromium tests check request isolation, ordinary
diagnostic equivalence and ABI-buffer retirement on success and rejection.
Zero ABI counters do not prove collection of every guest GC object.

V3 phases are `link` (1), `generics` (2), `gc-order` (3), `lower` (4),
`syntax` (5), `validate` (6) and `complete` (7). Older artifacts retain
`lower-render` / `reparse` for phases 4/5. GC ordering includes final
macro-query validation. Classified frontend limits are `instances` (1),
`elaboration-work` (2), `traversal-depth` (3), `gc-types` (4), `named-fields`
(5), `gc-edges` (6), `rendered-source` (7), `syntax-nodes` (8) and
`syntax-storage` (9) and `syntax-work` (10). Zero means no classified limit;
another parser/checker limit can still be identified by its phase and ordinary
diagnostic.

## Compare two compiler artifacts

Keep the before artifact, build the changed profiler into another directory,
and give both the same input:

```sh
just build-selfhost-profile target/profiler-before
# Change the implementation, then build a separate artifact.
just build-selfhost-profile target/profiler-after
just compare-selfhost-profile target/profiler-before target/profiler-after examples/selfhost/project/main.walz target/profile/comparison.json 9
```

The harness checks dependency/source hashes, validates both emitted Wasm
binaries, and requires byte equality before publishing a timing ratio. It warms
both compilers, alternates order across samples and records the median. Counters
must repeat deterministically on each request. If either compiler rejects the
input, or successful binaries differ, it records outcomes and withholds timing;
a binary mismatch also fails the command. Faster failure is not a speedup.

New reports use `walz-selfhost-profile-v4` and
`walz-selfhost-profile-comparison-v4`. `countersSchema` distinguishes v1/v2/v3/v4
compiler snapshots when comparing retained old artifacts. V2 adds layout/field
cache counters and records partial renderer progress; v3 adds direct syntax
assembly counters and distinguishes its phases; v4 adds sharing/visit counters.
Old measurements retain their
original schemas and values. Reports record input hashes, generated
Wasm/adapter/manifest hashes, recorder hashes, Node/V8/platform/architecture,
counters and elapsed times. Timed calls include JS ABI conversion and snapshot
lifting; file discovery, artifact inventory and output writing are outside the
timed region. These are whole-frontend timings, not timings of type resolution
alone. Keep inputs and both artifacts when preserving a measurement. For a
self-source comparison, snapshot the source project first so changing the
compiler does not also change the input being measured.

## Initial signature-cache comparison

Across the maintained v4 shared-syntax corpus, all 37 successful inputs produced
byte-identical Wasm before and after caching; all 19 rejected inputs retained the
same diagnostic and original range. Three fixed profiling inputs show the work
reduction:

| Input | Signature computations before → after | Return computations before → after | Type work before → after |
| --- | --- | --- | --- |
| 1,000 calls to one generic specialization | 1,000 → 2 | 2 → 2 | 3,009 → 1,005 |
| 100 calls with inferred let types | 100 → 2 | 102 → 2 | 509 → 205 |
| `examples/selfhost/project/main.walz` | 12 → 5 | 9 → 5 | 531 → 445 |

The first two inputs use `repeated` from the maintained v4 budget corpus; the
inferred variant wraps each call in `(@let $value ... $value)`. Initial local
whole-frontend timings were approximately unchanged. The counter reduction is
not evidence of a substantial end-to-end speedup.

In the initial v1 run on fixed pre-v5 compiler sources, the original profiler reached the
1,048,576-work limit after 20,563 signature computations. The cached compiler
advanced to diagnostic 110 at `(@bytes.from-array $Array $out)` in
`walz/core/bytes.walz`. An isolated uncached profiler with a larger experimental
work budget reached the same diagnostic and range, confirming that the new
stop was previously hidden by the limit. The work budget remains unchanged.
These different failure outcomes are progress evidence, not a timing comparison
or completed self-hosting.

The v5 frontend fixes that byte-conversion mismatch and adds initializer
nullability checks. A fresh complete-source probe creates 521 function and 302
heap instances before reaching the same finite work budget. This is a different
input/compiler capture, not a rerun of the v1 measurement. The unchanged GC
ordering limit of 128 generated types was another gate after expansion in that run.

```sh
just test-selfhost-profile
just test-selfhost-front
just test-selfhost-parity
just test-selfhost-e2e
```


## Layout reuse and sparse ordering, v6

A separate fixed-input capture uses `fields(1000)` from
`runtime/selfhost-front/scaling.mjs`: 24 struct fields and 1,000 field reads whose
let types are inferred. Before/after binaries are identical. Type work falls
from 27,707 to 3,155 visits; the composite layout and full field-type list are
each computed once. Nine alternating local samples gave medians of 47.27 ms
before and 20.28 ms after (2.33x). These whole-frontend measurements include
profiling/ABI overhead and describe this input on the recorded engine, not a
general compiler speedup.

The new ordering pass admits 1,024 GC types with at most 65,536 dependency edges;
the aggregate function/heap/alias cache admits 3,072 entries. Independent emitted
function/type limits stay at 1,024 and elaboration work stays at 1,048,576.
In the v6 complete-source capture, 700 function, 372 heap and 52 alias
instances reach ordering successfully with 646,016 charged visits. Serialization
then reports `rendered-source`, with `renderedBytes: 1048576`. This is a fresh
input/compiler capture; the earlier v1/v5 observations above remain historical.
See [ordering contracts](selfhost-ordering.md).

## Direct syntax assembly, v7

Compilation and macro queries now bypass intermediate text and use a checked
postorder arena. `renderedBytes` stays zero. Large decoded-string exports compile
and execute, and original UTF-8 error ranges remain checked. The same fixed
`fields(1000)` comparison retains identical Wasm; local whole-frontend timing
was approximately unchanged (0.98x across nine alternating samples), so this
change does not establish a speedup on that workload.

The v7 current-source probe created 695 function, 365 heap and 51 alias
instances with 645,221 charged visits, then reaches `syntax-nodes` at 262,144
nodes and 2,274,402 logical storage bytes. This is a different source/compiler
capture from v6, not a timing comparison of those failing runs. It removes the
intermediate-WAT stop but does not establish full self-hosting. See
[syntax construction contracts and limits](selfhost-syntax.md).

## Sharing and allocation tuning, v8

The [repeatable tuning loop](selfhost-tuning.md) compares frozen inputs across
separate artifacts. Short atoms share only when their spelling and complete
source identity match; fully validated indexed nodes avoid a second arena copy.
The larger specialization input improves by a repeatable 1.08–1.09x, with 57.4%
fewer arena nodes. A small project retains about 0.2 ms overhead; this is not a
general speedup. A sampled bucket-hash experiment was measured and reverted.

The v8 source probe has 702 function, 368 heap and 51 alias instances
with 646,073 charged elaboration visits. Its 537,068 syntax visits and 296,186
atom hits produce 240,882 nodes and 2,481,495 charged storage bytes. Independent
validation then rejects unsupported `@abi.type` metadata in the diagnostic
record (code 101). Timings of differently failing compiler probes remain withheld.

## Checked ABI metadata, v9

The [source ABI pass](selfhost-abi.md) validates supported record/tuple views
before specialization. Profiler v4's schema is unchanged: ABI resolution has a
separate bounded counter, and the elapsed frontend call includes that work.

The v9 source created 747 function, 398 heap and 55 alias instances, with
658,586 charged elaboration visits. Syntax assembly completes with 248,369 nodes,
549,199 logical visits, 300,830 atom hits and 2,546,497 logical storage bytes.
Its next error was 118 in independent GC type validation, attributed to a match
in `frontend/modules.walz`, original UTF-8 range 35,312..35,753. It is not a
resource-limit rejection or a completed self-compilation at that revision.

A nine-sample alternating comparison against the retained pre-ABI artifact,
using the existing frozen tuning inputs, preserves all five successful Core
binaries byte for byte. Generic lists measure 228.98 → 229.71 ms; the small
project measures 4.49 → 4.89 ms. This is an added checked feature, with no
measured speedup claimed. These local samples do not establish stable overhead.
Reports are under `target/selfhost-abi/tuning/`; the failed compiler subject has
no timing ratio, and earlier captures remain unchanged.

## Self-compiled stages, v10

The control/inference fixes resolve the former validation boundary. The current
subset self-compiles and passes exact Core stage 1/2 and portable-output equality;
see [the fixed-point gate](selfhost-stages.md). That gate retains input bytes,
labels, output hashes and original diagnostics for the complete maintained corpus.

The profiling schema remains v4. Stage elapsed values include different host
transports and cold calls, so the gate does not report them as speed ratios.
The separate [tuning harness](selfhost-tuning.md) uses identical frozen subjects
and requires matching successful output before measuring a ratio.
