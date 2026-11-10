# Macro usability v1

Assess whether Walz's declarative macros help with ordinary application development, using the current compiler without repairing it during the run.

1. Implement the same JSON expense validator in three forms: built-in Walz expressions, ordinary helper functions, and user macros. Share the nominal data model, aggregation and output code. Validate a string `category` and a lexical signed-64-bit integer `amount`; reject overflow without committing the row. A policy-change task additionally rejects negative amounts.
2. Compare all three implementations with an independent exact-integer oracle on empty, malformed, missing/wrong fields, signed bounds, duplicate keys, Unicode, embedded escapes and a larger mixed batch. Run Core+JSPI and Component, checking ABI buffers and live endpoint tables after each call.
3. Exercise intentional binding, definition-site helpers, inferred numeric types, evaluation order and normal/queued/running cancellation across a macro expansion. Verify source locations and current-revision responses through the public stdio LSP protocol.
4. Attempt a flat repeated-field-binding DSL and a binder-forwarding macro. These are desired authoring tasks, not promised capabilities. Preserve failures and diagnostic codes instead of weakening the task or silently replacing it with nested syntax.
5. Measure consumer and total source tokens/depth, generated Wasm bytes, balanced repeated CLI check/build elapsed time, warm call latency, and synthetic scaling from 1 to 256 call sites. Report failed compilation and limits as observations. CLI elapsed time includes process startup and loading; it is not an isolated type-checker measurement.

The evaluator knows the implementation. Rubric judgments are implementer dogfooding, not a blind reader experiment or a human productivity measurement. A smaller source file alone does not establish readability. Compare the function alternative before attributing savings to macros.

Each run must have a fresh ID. Freeze this contract, rubric, Walz/JS inputs, docs and compiler/runtime fingerprints before executing. Do not alter frozen inputs, prior results, or expected values. Record required-check failures separately from exploratory unmet tasks. No compiler or runtime fixes are part of this evaluation.
