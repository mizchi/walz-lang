# Execution Through Ownership Checking and Proofs

```wat
(@let $ticket (@box.new i64 (i64.const 42))
  (@match (@inst $box i64) (result i64) (@move $ticket)
    (@case own ($value) (local.get $value))))
```

`(@inst $box T)` is a non-copyable wrapper. Reuse after move, implicit abandonment, and storage in shared GC fields are rejected.
Ownership also propagates through Option/Result/pairs and capturing closures. Explicitly consume an unused wrapper with `(@box.close (@move $ticket))`.
Extracting contents through match also consumes it. Own does not make a payload's GC references exclusive, and own.close does not yet implement external-resource shutdown.
There are no general lifetimes; operations receiving owned values return them as part of their result. External resources have non-escaping borrows and synchronous destructors. See [ownership.walz](../examples/ownership.walz) for pairs and once-call closures.

```wat
(@law "adding zero preserves the value" (param $x i32)
  (i32.eq (call $add (local.get $x) (i32.const 0)) (local.get $x)))
```

A law requires its condition to be nonzero for all i32/i64 inputs; Lean checks a BitVec proof generated from the actual function body.
This includes integer wrapping, signedness, and Wasm shift-count masking. The initial law implementation is restricted to pure integer expressions and non-recursive direct calls.
Changing a function body so that a law fails also rejects ordinary run/build. An unavailable verifier or unsupported semantics is an error too.
[verified-laws.walz](../examples/verified-laws.walz) combines an addition law with owned values.

`run-verified` also passes ownership contracts to an independent Lean checker, then generates Wasm from the same IR that passed.
This mode rejects unanalyzed WAT, imports, raw exports passing owned values, and public functions without effect bounds. Ordinary WAT acceptance remains available.
The Rust `check` API and inspection commands types/ir/analyze do not prove laws. CLI check and executable generation check proofs when laws are present.
The formalized scope and trusted implementation are documented in [SPEC.md](../SPEC.md#law-と契約付き実行) and [proof/README.md](../proof/README.md). This does not prove whole-compiler semantics preservation or termination.

Ownership states for exceptions, return, and labeled branches are also checked through version 2 control-flow certificates. Nominal types are checked before executable newtype erasure; integer laws prove properties of the erased representation. See [supported scope](core-expressions.md).
