# Bootstrap compiler tuning loop

The tuning harness freezes source inputs once and compares separate compiler
artifacts against them. It measures the complete Core+JSPI frontend call,
including JavaScript ABI conversion and profile lifting. Dependency discovery
and report writing are outside the timed call. Component and Chromium are
correctness gates here; these timings do not measure their performance.

## Repeat the loop

```sh
just build-selfhost-profile target/tuning/before
just selfhost-tuning-prepare target/tuning/before target/tuning/inputs
# Change the implementation; keep the before artifact and input snapshot.
just build-selfhost-profile target/tuning/after
just bench-selfhost-tuning target/tuning/before target/tuning/after target/tuning/inputs target/tuning/run-01 15
just bench-selfhost-tuning target/tuning/before target/tuning/after target/tuning/inputs target/tuning/run-02 15
just test-selfhost-tuning
```

Input/output directories must be new. The harness rejects changed hashes,
unexpected files and paths escaping the snapshot. Each run keeps an input
manifest hash and per-case reports containing source, Wasm/adapter and recorder
hashes, engine/platform, raw samples and counters. An optional source root can
be supplied to `node runtime/selfhost-tuning.mjs prepare COMPILER INPUTS ROOT`.
This is useful for retaining a pre-edit compiler source as the subject.

Tuning inputs v1 contain `fields(1000)`, 128 nominal list specializations, the
example project, the number parser, the graph library and the complete compiler
source. Generators live in `runtime/selfhost-front/scaling.mjs` and `tuning.mjs`.
Prepare once: preparing again after changing compiler source creates a different
subject and must use a new capture.

Each successful compiler is warmed and its output is independently validated.
Timing ratios require byte-identical Wasm. Samples alternate before/after order;
the report uses medians. An accepted baseline becoming rejected fails the suite.
Rejected subjects still retain their diagnostics/counters, with timing withheld.
Finite-budget failures cannot be compared as a speedup.

## Measured iterations

The local capture used Node 24.22.2 on darwin/arm64. All rounds used input
manifest SHA-256 `3eff246764228b167ab123fd33c2fb90401c102fdc2fb21a8ddd4c4340300ce4`.
Raw artifacts and reports are under `target/selfhost-tuning/`: `before`,
`iteration-1` through `iteration-4`, `snapshot`, and `run-1` through `run-4`.
`confirm-3` repeats the adopted third candidate; `final` rebuilds that candidate.
These are new tuning captures, not replacements for historical scalar benchmarks.

| Round | Change | Samples per side | Generic-list ratio | Small-project ratio | Decision |
| --- | --- | ---: | ---: | ---: | --- |
| 1 | Same-origin short-atom sharing | 9 | 1.08x | 0.94x | Keep sharing and investigate overhead. |
| 2 | Eliminate empty child builders for leaves | 15 | 1.08x | 0.97x | Keep the allocation reduction. |
| 3 | Reuse fully validated indexed nodes/arenas | 15 | 1.09x | 0.94x | Adopt; repeat below. |
| 3 repeat | Same artifacts and input hashes | 15 | 1.08x | 0.95x | Confirms the workload-specific result. |
| 4 | Sample spelling bytes in the bucket hash | 15 | 1.09x | 0.94x | Reverted; it did not remove the small-project cost. |

The adopted candidate's repeat (`confirm-3`) gives:

| Input | Before median | After median | Ratio | Arena nodes before → after |
| --- | ---: | ---: | ---: | --- |
| 128 nominal list specializations | 247.02 ms | 227.87 ms | 1.08x | 36,369 → 15,489 |
| 1,000 inferred field reads | 21.27 ms | 21.00 ms | 1.01x | 14,157 → 10,153 |
| Small project | 4.18 ms | 4.39 ms | 0.95x | 614 → 484 |
| Number parser | 4.69 ms | 4.57 ms | 1.03x | 1,303 → 1,012 |
| Graph library | 3.96 ms | 4.02 ms | 0.99x | 1,300 → 1,056 |

The larger specialization case repeatedly measures 1.08–1.09x (about 8% less
frontend time), while
the small project costs about 0.2 ms more. The other small timing changes do not
establish useful speedups. The specialization arena has 57.4% fewer nodes;
logical charged storage falls from 293,023 to 168,393 bytes (42.5%). Logical
storage is not a measurement of GC heap or peak process memory. The profiling
artifact's Wasm grows from 302,883 to 305,305 bytes (0.8%); emitted subject Wasm
is identical.

The frozen complete-compiler subject advances from a partial 262,144-node budget
failure to independent validation: 535,468 logical visits produce 239,586 nodes
and 295,882 atom hits. Its next rejection is unsupported `@abi.type` metadata.
The old partial and new completed arena sizes are not a percentage reduction of
the same completed work, and this failing subject has no timing ratio.

## Contracts and remaining work

Sharing keys include complete byte spelling, file, start and end. Full equality
is checked after hashing; distinct source origins stay distinct. Forms and
strings are not interned. The cache belongs to one request, grows within the node
budget and does not turn repeated references into eliminated instructions.
Logical visits, including hits, have an independent finite limit. Indexed-node
reuse follows full structural/token/depth/storage validation and exact index
verification; the independent type/effect checker still runs.

The retained v8 checks cover 65 positive and 31 negative inputs. The v9
corpus adds [checked ABI views](selfhost-abi.md), with 76 positive and 64 negative
shared Rust/bootstrap cases. An unchanged-input before/after gate
also retains binaries/diagnostics for 245 frontend cases, independently validating
successful binaries. Cache growth, source distinctions, long uncached atoms,
visit boundaries, invalid index hints, both adapters and buffer retirement are
covered by `test-selfhost-syntax`; snapshot integrity has its own test.

The v9 full source validated its supported ABI views, completed assembly with
248,369 nodes and reaches independent GC type validation (118), attributed to a
match in `walz/frontend/modules.walz`. The short-project overhead remains a TODO.
Self-compilation was incomplete at that revision. The v10 subset now passes
[Core stage and portable-output equality](selfhost-stages.md), while the
short-project fixed cost remains a tuning target.
The [v9 profile note](selfhost-profiling.md#checked-abi-metadata-v9) records a
separate same-input comparison and its observed costs without replacing v8 data.

## Self-compilation follow-up (v10)

The v10 control/type fixes make the full compiler subject succeed. The earlier
snapshot remains unchanged. A separate successful-input snapshot has manifest
SHA-256 `76b5231695ecb96179285bf1142796c6947d4b803b2a7d3fcb7e59bd65500ac0`;
it includes the compiler's corrected unit statement blocks. Captures are under
`target/selfhost-validation/`: `inputs-complete`, `tuning-complete-1`,
`tuning-complete-repeat`, `tuning-final`, `tuning-adopted` and `tuning-clean-final`.

The retained implementation uses exact ASCII dispatch for scalar aliases,
shares comparison classification with that table, and substitutes a function
result payload without first copying its wrapper. An extra loader ABI flag was
tried and reverted after repeated runs showed no consistent benefit.

The final quiet comparison used Node 24.22.2 on darwin/arm64, three measured
samples per side after warmup, and identical validated output bytes for all six
subjects:

| Input | Before median | After median | Ratio |
| --- | ---: | ---: | ---: |
| 1,000 inferred field reads | 23.23 ms | 24.32 ms | 0.96x |
| 128 nominal list specializations | 239.00 ms | 239.91 ms | 1.00x |
| Small project | 5.53 ms | 5.11 ms | 1.08x |
| Number parser | 6.14 ms | 6.11 ms | 1.01x |
| Graph library | 5.16 ms | 4.64 ms | 1.11x |
| Complete compiler | 10,589.77 ms | 10,790.48 ms | 0.98x |

These short runs varied across repetitions and do not establish a stable
end-to-end speedup. The earlier 15-sample small-project comparison was 1.02x;
the complete-compiler follow-ups ranged from 1.09x to below 1.00x. The small
project's fixed cost remains open. Keep these measurements separate from the
earlier v8 specialization result.

The `compiler` timing in `tuning-adopted` overlapped a focused test/build and is
excluded from performance conclusions. Its raw capture is preserved, with
`tuning-adopted-timing-note.json` recording the overlap and naming
`tuning-clean-final` as the replacement. Stage verification uses separate cold
calls through different transports; its elapsed times are not speed ratios.
