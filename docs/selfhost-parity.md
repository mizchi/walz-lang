# Rust / bootstrap feature parity

The goal is to compile the same supported Walz source independently and obtain
matching observable behavior and checked contracts. The bootstrap compiler must
not fall back to the Rust compiler for parsing, expansion, type checking or code
generation. Unsupported safety/ABI contracts remain errors until their validators
and lowering exist.

## Completed frontend additions

- [x] Function/type import aliases and explicit reexports, preserving declaration
  identity, opaque access and the original native function/type index spaces.
- [x] Built-in Add/Sub/Mul/Div/Eq/Ord/Logical/Bitwise/Shift constraints for the
  current i32/i64 subset. Logical accepts only i32. Macro guards and generic
  constraints use the same implementation. Empty/unknown clauses are errors.
- [x] Named struct construction with `@struct.new`, curly field groups and
  `@$Type::{...}`, including generic types. Evaluate each initializer once in
  written order, then construct in declaration order. Reject missing, duplicate,
  unknown and anonymous fields, foreign opaque layouts and type mismatches.
- [x] A maintained differential corpus: compile identical shared-syntax inputs
  with Rust and the bootstrap compiler, validate and execute both binaries, and
  require rejection of negative cases. Bootstrap native-index regressions are
  separate because the Rust typed `@use` path requires named references.

- [x] Bounded large-definition caches, valid unit returns and flat short-circuit
  chains. Check distinct definitions versus repeated calls, original evaluation
  order, skipped traps, rejected skipped type errors and finite budgets.

- [x] Request-local frontend counters, file-level work attribution and classified
  expansion limits. Cache resolved function signatures and return types without
  caching failures or crossing nominal/opaque boundaries. Preserve selected
  comparison binaries and ordinary diagnostics. See [profiling](selfhost-profiling.md).

- [x] Checked i8/i16/i32 array-to-bytes snapshots, selected-range checks,
  snapshot independence and source-order evaluation. Reject nullable bindings
  before nullable physical storage; preserve subtyping and loop branch depth.
  Execute the maintained production `bytes.concat` source with the bootstrap.

- [x] Concrete layout/field reuse and sparse iterative SCC ordering. Bound the
  graph at 1,024 vertices/65,536 edges, retain explicit-rec groups and inheritance
  order, reject parent cycles and preserve original numeric type references.
  Independent graph oracles cover 1,024-node chains/rings in both adapters.

- [x] Direct, bounded postorder syntax assembly for compilation and macro query
  validation. Preserve decoded bytes and original UTF-8 source ranges; retain
  the independent checker. See [syntax assembly](selfhost-syntax.md).

- [x] Request-local same-origin atom sharing, bounded logical visits and validated
  indexed-arena reuse. Keep byte-identical output and diagnostics; test cache
  growth, source distinctions, invalid index hints and success/rejection cleanup.

- [x] Checked closed record/tuple ABI views, trusted core carrier identities,
  memoized shape/layout facts and bounded expansion. Preserve opaque ownership,
  original annotation ranges and independent instruction checking. See
  [the ABI contract](selfhost-abi.md).

- [x] Empty result vectors, statement/condition/result contexts, bare variable
  conditions and numeric literals distinct from explicit local indices. Infer
  bitwise/shift aliases and string/byte values; retain independent rejection of
  bad operands and unsupported multiple results.
- [x] Self-compile the current Core compiler and portable entry; compare stage
  1/2 binaries and run the maintained corpus against both. Rust still builds
  the seed and host adapters. See [the fixed-point gate](selfhost-stages.md).

## Remaining work in dependency order

Each item needs positive execution, negative validation and original-range tests.
ABI changes also need successful, exceptional and cancelled cleanup checks.

- [ ] Complete the source forms: user enums/match, field patterns and updates,
  newtypes, closure/callable constraints and the remaining reader/Watz syntax.
- [ ] Complete scalar and SIMD instructions: exact float-literal parsing,
  rounding/NaN/signed-zero behavior, conversions, v128, block parameters and
  multiple results. Extend the independent IR validator and binary emitter.
- [ ] Add memories, globals, tables, imports, function/external references and
  flat WAT; preserve native indices, initialization order and shared identity.
- [ ] Complete recursive GC type equivalence while retaining nominal/opaque
  boundaries and rejecting malformed subtype declarations.
- [ ] Port the production core APIs: dictionaries, views, string/byte operations,
  JSON and collection libraries. Measure representation costs independently;
  the current balanced-tree bootstrap core has different storage/layout costs.
- [ ] Port full effect, ownership/borrow/resource and independent proof checks.
  Do not erase moves or accept unsupported owned carriers through macros.
- [ ] Add exceptions, async/Future/Stream/task cancellation and join semantics.
- [ ] Add Core+JSPI/Component ABI generation, WIT, explicit constructors,
  `instantiate`/`@connect`/`@expose`, resource identities and memory inspection.
- [ ] Add threads/atomics, shared-memory ownership and restricted race contracts.
- [ ] Port optimization and advisory passes with behavior-preservation checks;
  preserve traps, float rules, effects, reference identity and cleanup.
- [ ] Expand diagnostics/type information and CLI/LSP/ESM/Playground integration.
- [ ] Extend control-flow parity, including semantic-never propagation through
  value controls while preserving declared signatures and checked branch targets.

## Running the current gates

```sh
just test-selfhost-parity
just test-selfhost-front
just test-selfhost
just test-selfhost-e2e
just test-selfhost-bench
```

`runtime/selfhost-front/parity.mjs` is part of the maintained frontend correctness
corpus v10. It currently compares 107 successful inputs and 68 rejected inputs
with Rust. Resource-limit cases and WAT cases with numeric local indices,
labelled if or multiple statements in a branch remain separate; Rust's typed
`@use` path does not yet accept those forms.
The ordinary corpus also checks both host adapters, UTF-8 ranges and
ABI retirement. Recorded scalar benchmarks retain their original inputs and
contracts; a new correctness corpus is not a new performance measurement.

Full feature parity is still incomplete. See [the frontend limits](selfhost-frontend.md)
and [independent checker/codegen contracts](selfhost.md) for the present boundary.

The earlier array-to-bytes, ordering, syntax-assembly and source-ABI boundaries
are resolved. The current bounded compiler self-compiles, with byte-identical
stage 1/2 and portable outputs. Its 184 positive and 148 negative frontend cases
also agree across the seed and both self-compiled stages. Checked source views
and the Core GC test bridge do not implement WIT or general host adapters.
See [the stage gate](selfhost-stages.md), [checked ABI views](selfhost-abi.md)
and [tuning measurements](selfhost-tuning.md).
