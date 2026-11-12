<a id="契約検証の範囲"></a>

# Scope of contract verification

English | [日本語](README.ja.md)

The ownership source of truth is the contract in [SPEC.md](../SPEC.md#所有値の最小規約). Typed IR, extracted ownership contracts, and Lean rules are separate so implementation and contract changes can be reviewed independently.
The design was informed by [Bend 2's law/proof and usage-count conventions](https://github.com/bendlang/bend/blob/3ca3df3209ebb0fac4b80e41dac18b41442e6c58/guide/GUIDE.md). It is not a port of BendTT.

```sh
just proof-check
just verify examples/verified-laws.walz
just run-verified examples/ownership.walz
just run-verified examples/ready-delivery.walz
just contracts examples/ownership.walz
just proof examples/verified-laws.walz
```

Lean is pinned to `leanprover/lean4:v4.34.1`. Rust's verified path generates proof source from compile-time-embedded Ownership.lean and typed IR.
If Lean is unavailable or a proof fails, Wasm is neither emitted nor executed. The ordinary WAT path remains separate from this verification mode.

<a id="所有権"></a>

## Ownership

Version 1 contract operations are nop, take, scope, seq, branch, loop, again, and stop. Existing type checking handles shared-value types; contracts retain only uses of owned local IDs.
Declarative `Valid` relates available ownership sets, sets required on loop backedges, operations, and sets remaining on normal completion.
`check_sound` proves that successfully checked contracts satisfy this relation. `double_take_rejected` proves for arbitrary input sets that the same consumed slot cannot be consumed again.
Loops are not executed recursively. Checks compare entry state with the state returned by again. They guarantee neither termination nor eventual close.

Positive witnesses accept contracts consuming the same owned value once in each branch. Negative witnesses reject double consumption, branches consuming on only one side, loops losing outer owned values, and scopes exiting without consumption.
Examples within Lean files and `tests/verified.rs` check these with an independent checker; `tests/ownership.rs` additionally checks rejection of corresponding real walz code.
No `sorry` or custom axioms are used. Lean's standard logic and kernel are trusted.

<a id="分岐return例外の証明書"></a>

## Certificates for branches, return, and exceptions

Version 2 uses `graph` instead of `body` for functions with the new control flow. Nodes record available slot sets, nop/bind/take/absent/finish operations, and successor nodes. Slots include owned values during argument evaluation. Rust extracts normal/exceptional edges from inferred effects and enclosing handlers/labels. Lean checks that each edge's post-transition ownership set matches its successor's entry set and that finish has an empty set. It also rejects nop nodes with inputs but no successor, reachable unannotated nodes, and invalid indices.

`stepCheck_sound` proves correspondence with declarative `StepValid`; `graph_node_sound` proves matching states for every successor; `graph_path_sound` proves that annotations exist along paths including arbitrarily many iterations. Forged generated JSON examples are also rejected independently through `tests/verified.rs`. Version 1 functions can coexist in the same file.

Completeness of exception-path extraction and effect inference, correspondence with IR, borrow non-escape, using finally, and newtype erasure remain in the compiler TCB. See the [language conventions](../docs/core-expressions.md).

<a id="アプリケーションの-law"></a>

## Application laws

Laws and the actual typed IR of their dependencies are converted to BitVec definitions. Arguments are universally quantified, and bv_decide generates/checks proofs that the law's i32 result is nonzero.
This does not select a finite set of test inputs. It checks properties over every i32/i64 bit pattern, including fixed-width wrapping.
`tests/laws.rs` verifies that execution is rejected when addition is changed to subtraction and for a law failing only at x=65537.
Signs, extensions, shift-count masking, branches, and hexadecimal wrapping are checked in both the checker and Wasm execution.
Currently only pure, nonrecursive integer fragments are supported. Recursion, loops, division, GC, owned values, floating point, and similar features are rejected as unsupported rather than filled in with assumptions.

<a id="配送済み-futurestream-のモデル"></a>

## Model of already-delivered Future/Stream

Delivery.lean defines ready/closed Futures and open/closed Streams with finite sequences.
It proves that a Future cannot be read again after await, closed Streams cannot be read, terminal reads still return an open endpoint, and repeated next matches take/drop of the original sequence.
Stream EOF does not affect a separate completion Future's Result.
This model is also included in Lean source for contract-checked execution. Existing ownership contracts check read/close usage counts.
`tests/delivery.rs` and `tests/programs/delivery.test.walz` execute corresponding real code with Wasm GC, comparing order, rereads, unit, GC references, evaluation counts, and termination.
`tests/wit.rs` actually encodes/validates conversion to WIT future/stream.
The model is ready-only. It does not prove pending behavior, writers, backpressure, external I/O, or producer-task cancellation.

<a id="解析で用いるビット条件"></a>

## Bit conditions used in analysis

Flow.lean checks over all 32-bit BitVec values that adding a non-negative condition to a signed index comparison implies an unsigned bounds condition.
It also checks the counterexample where -1 passes a signed comparison without non-negative, and addition wrapping. `just proof-check` verifies this independently.
`tests/flow.rs` checks that analysis does not attach bounds_proven to the same negative index and actual Wasm traps out of bounds. It also compares interval results and concrete wrapping operations.
This lemma does not prove soundness of Rust's abstract interpreter. memory/facts currently serve only diagnostics and are not yet grounds for eliminating generated Wasm instructions.

Ranges.lean separately proves unsigned one-byte bounds, composition of validated
subranges using natural-number subtraction, and absence of i32 offset wrapping
under those premises. A BitVec counterexample rejects a wrapped-sum guard alone.
The [must-range optimizer](../docs/range-optimization.md) uses these arithmetic
rules independently of diagnostic facts. The proofs do not establish correctness
of Rust extraction, inlining, control-flow interpretation, or Wasm lowering.

<a id="信頼する実装"></a>

## Trusted implementation

Tasks.lean is a state model separating JSPI task requests from completion confirmation. It proves pre-execution cancellation, request idempotence, ineffectiveness of cancellation after return, rejection of buffer release before reads finish, and rejection of completion confirmation while resource/host cleanup remains.
A witness rejects completion confirmation with disposals=1 and accepts it after setting disposals to 0. Corresponding Node tests also hold dispose at a barrier and check that the cancel Promise remains unsettled meanwhile.
This model is also included in generated Lean source. It does not prove refinement to Rust/JS/Wasm, time until cancellation completes, or stopping external producers.

The proofs cover the contract models above and generated BitVec definitions. The following are outside their scope.

- Parsing, name resolution, Rust type checking/effect analysis/ownership-contract extraction, and reference/escape/control-flow analysis.
- Correspondence between typed IR and Lean BitVec definitions, and semantic preservation of lowering to Wasm.
- Lean/Wasm engines, external-resource operation protocols, Component ABI adapters and buffer allocation/release, jco JSPI glue, and localized compatibility fixes.

Proofs and executables come from the same checked IR; regression tests retain both invalid contracts and actual code to detect discrepancies across this boundary.
These proofs do not establish GC lifetime/OOM behavior, runtime traps, server termination, or Future/Stream completion/cancellation.
Component generation checks the same ownership contracts and laws, but Delivery.lean itself is ready-only. External pending handles, Canonical ABI conversion, and JSPI suspension/resumption are compared through Node and Chromium execution tests.

bytes/text UTF-8 validation, private storage, recursive Canonical ABI copying/release, static namespace/use expansion, and the include file loader are also in the TCB. Their semantic preservation is not proved by the Lean kernel.

Lean counter models alone cannot check actual JS-reference retention. `just memory-check` dynamically checks remaining ownership-table entries, GC reachability, repeated-execution retention, and allocator reuse. Observations and scope are recorded in [MEMORY.md](../MEMORY.md).
