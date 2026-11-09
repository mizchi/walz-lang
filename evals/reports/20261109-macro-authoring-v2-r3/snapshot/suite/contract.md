# Macro authoring and scale v2

Freeze all inputs, expected values, documents, harness and implementation fingerprints before executing. Never repair the compiler or runtime during this run. Use a fresh run ID. Preserve v1 and all prior results.

Required tasks: infer an ordinary generic call; define an optional macro without internal type-pattern knowledge; use public option.element queries; compose independent repeated bindings; preserve caller scope in initializers; reject duplicate binders; and handle 32/128/256/512 calls split across independent functions plus 32 calls in one function. Execute each accepted program's frozen main/fallback results on Core Wasm in Wasmtime and V8. Compilation success alone is insufficient. These tiny programs do not measure ABI cleanup or cancellation; pair the run with the unchanged v1 suite for those checks.

Exploratory tasks: let-like sequential dependencies between captured initializers, repeated Result.do bindings, wrapping a delegated body, and 128/256/512 calls in a single function. Some intentionally exceed documented capabilities. Rejection is an unmet authoring task, not automatically a compiler bug. Wrong results from any accepted program always fail correctness, including exploratory programs.

Check each case three times; preserve every exit code/diagnostic and median elapsed time. Elapsed CLI time includes startup/loading. For accepted cases record expansion work counters and execute all exports with fixed values. Preserve errors and do not weaken tasks to make them pass. Verify unchanged frozen inputs and implementation at the end.

Compare the three option-to-result variants by both call-site and helper-definition syntax. Runtime/code size and token counts alone do not establish readability or justify a macro. This is nonblind implementer evaluation, not an independent reader or human productivity study. v2 adds different tasks, so do not compare rubric point deltas with v1. Give each axis evidence and never average away a weak axis.
