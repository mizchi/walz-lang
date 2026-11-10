<a id="実用化に向けた実装順"></a>

# Implementation order toward practical use

1. Differential testing, fuzzing, and CI. Check seeds, optimization on/off, macro/explicit versions, Wasmtime/Node, side-effect order, cancellation, and cleanup; provide reduction, storage, and replay of failures.
2. Generate WIT bindings for existing value types. Obtain types from WIT and effects/capabilities from a separate trusted policy.
3. Typed external resources and explicit close. Check using scopes and exit paths for success, Result.err, exceptions, and cancellation.
4. Small simplifications of typed IR. Remove unnecessary boxes for values immediately matched, preserving evaluation order and ownership.
5. Complete standard library JSON values and parsing, and share list map/fold through explicitly monomorphized generics. Do not introduce type-argument inference or traits.

<a id="差分検査"></a>

## Differential testing

`just fuzz SEED CASES` generates finite typed expressions. It independently computes operation results and hashes of host call order, comparing macro/explicit versions and optimization on/off in Wasmtime and Node.
It also checks mutations of invalid sources, treating compiler panics as failures. Failures are saved under `target/fuzz/failures/`.
Saved expression JSON and invalid .walz inputs can be replayed with `just fuzz-replay FILE`. It reduces subtrees including expression payloads and strings of invalid input. This is not a soundness proof for the entire language or all inputs.
`runtime/fuzz.test.mjs` selects queued/pending/completion cancellation timings and microstep order from a seed, checking return values, cancellation reasons, double close, and remaining management-table entries.

CI runs two jobs, Rust/Lean/Wasm CLI and JS/Chromium, and saves failing inputs and browser logs as artifacts. Tool versions and distribution hashes are pinned in `ci/tools.json`.

<a id="直後の-match-の簡約"></a>

## Simplifying an immediate match

When a shared Result/Pair is constructed and immediately matched, transform it into the selected arm and a sequence of payload lets.
Evaluate even unused payloads once each, left to right, preserving traps and side effects. Owned payloads are excluded.
The original source's type, effect, and ownership checks and any required Lean checks run first. IR/analysis retain the original source.
`compile_with_options(..., CompileOptions { optimize: false })` provides the comparison baseline.
This pass and reachability elimination are part of the compiler TCB; they do not constitute a Lean proof of semantic preservation.

Even when unreachable suspends are removed, public functions retain the async ABI from the original analysis.
Unused host imports are removed without changing the JS Promise contract.

Equivalence under resource limits, such as allocation failure for eliminated temporary GC boxes, is outside the scope. Traps in payloads themselves are preserved.
