# Selfhost compiler benchmark and output parity

The [bootstrap compiler](selfhost.md) has a versioned benchmark for its closed
integer subset. The [v2 measurements](../benchmarks/selfhost/20261113-v2/README.md)
compare resident Core/JSPI and Component compiler calls against two optimized
Rust references. The [initial v1 run](../benchmarks/selfhost/20261113-v1/README.md)
is retained; v2 adds the narrower Rust assembler/validator reference without
changing source inputs or probes.

These saved runs predate GC type/checker/codegen support. They describe the
captured scalar compiler, not the size or speed of today's expanded backend.
Keep the captures unchanged and use new output directories for new measurements.

For browser deployment costs, see [playground compiler size](playground-size.md),
which includes JS adapters and browser-requested Wasm assets.

## Reproduce

From the repository root, with Rust 1.99 and Node.js 24+:

```sh
just test-selfhost-bench
just capture-selfhost target/selfhost-bench/my-capture
just verify-selfhost target/selfhost-bench/my-capture
just bench-selfhost target/selfhost-bench/my-capture target/selfhost-bench/my-run
```

Capture builds the release-mode Rust peer and both guest adapters, freezes source
inputs, expected execution probes, outputs, tool/runtime artifacts and source
hashes, then verifies all compilers. Measurement consumes that snapshot and does
no builds. Both commands refuse to replace an existing directory. Failed runs do
not publish successful `results.json` files. Use a fresh directory to rerun.

The compact public run record retains input/probe files, all generated modules,
canonical modules, raw samples, hashes and code snapshots. The complete local
capture also retains the executable Rust peer and both runtime adapters. Replay
that exact capture through its frozen `source/runtime/selfhost-bench.mjs`; the
compact record's code copies document provenance and are not a standalone package.
Building a fresh capture measures the current source, rather than recreating a
historical binary from only its commit name. Uncommitted source is hashed too.

## Equality contract

Before timing each capture, the harness compiles every input with the full Rust
compiler and both guest backends. A separate Rust tool using the pinned
`wasmparser` and `wasm-encoder` crates:

1. Validates each complete output module.
2. Rejects sections, value types and instructions outside the closed subset.
3. Re-encodes the supported module into a canonical binary and requires exact
   byte equality with the Rust output's canonical binary.

Canonicalization removes only `name` debug sections, unused/type-interned scalar
signatures, local grouping and alternative LEB spellings. It preserves function
order, expanded signatures, local order/types, **every instruction and immediate**,
and export names/order/indices. It does not remove unreachable instructions,
fold constants, reorder expressions or ignore extra code. Imports, memories,
globals, tables, start functions, GC, floats, multi-value results, type-indexed
blocks, unknown custom sections and unknown instructions fail closed.

For this closed subset, equal canonical modules have the same executable Core
Wasm definitions for all function arguments, including branch/trap behavior.
This is stronger than matching a finite list of return values. Raw `.wasm` files
can differ: debug custom sections and their introspection are explicitly outside
this contract. It is not a formal proof of either compiler's correctness for all
source programs, a claim about physical resource exhaustion, or a full-Walz
translation-validation system.

Each capture also executes independent probes against both guests and Rust,
covering i32 wrapping, i64 extremes, recursion, loops, conversions, lazy branches
and arithmetic traps. Every *measured* compilation must reproduce that compiler's
captured raw bytes. Changed bytes, a failed check or changed function count abort
the run. Snapshot hashes are checked before and after measurement. Thus a faster
incorrect compiler cannot be recorded as a successful result.

The eight inputs include a small integer expression, i64 loop, recursive calls,
branches/traps, i64 bit operations, 194 functions with wide indices, and generated
32/128-function programs. The largest is about 35 KB. General Walz macros,
lexical let, imports and GC types are outside this parity suite; accepting them
requires a new, justified equality contract and a new suite version.

## Timing method and interpretation

- Full Rust uses `walz::compile`; its check row uses `walz::validate`, matching the
  API that accepts native WAT as well as structured Walz. The i64 local/loop input
  uses its native-WAT fallback; `structuredCheck` records that distinction.
- The narrower Rust reference uses `wat::parse_str` followed by full
  `wasmparser::Validator` validation. Its raw output must equal the full Rust
  compiler's output. It has compile rows only and does no Walz source analysis.
- Both Rust references run in one persistent **release** process. JSON transport
  is outside its internal monotonic clocks. Guest timings include the complete
  value ABI, input copying, parsing, checking, encoding, output lifting and
  allocation-triggered GC.
- Each call is timed individually; its output is checked outside the clock. Two
  clock reads remain in each call's time. Each sample is the mean of calibrated
  calls, and the reported value is the median of nine sample means. Raw samples
  are retained without outlier removal.
- Each variant gets at least 300 ms / 16 calls of warmup. Variant order rotates
  each round. Node GC and microtask draining occur outside sampled batches.
  Startup, loading, instantiation, build/file I/O, calibration and verification
  are excluded. Run without concurrent builds, tests or profiling.

Rust's full API performs additional analyses that this scalar guest compiler does
not implement. A speedup against that API therefore does not establish that Walz
is faster than Rust, or that complete selfhosting would retain the speedup. The
assembler/validator reference makes that overhead visible. These are source
compilation timings; generated-program execution speed is not being benchmarked.

## Regression checks

`just test-selfhost-bench` verifies canonicalization boundaries, rejects changed
outputs/snapshots, and compiles the entire corpus through Core and Component.
The Rust integration tests are also discovered by `just test`; Node corpus tests
are included in CI's `runtime/*.test.mjs` glob. Tests include differences hidden
behind unreachable code and export ordering so canonicalization cannot silently
weaken the executable contract.
