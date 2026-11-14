# Self-compiled Core stages

The bounded compiler written in Walz compiles its own supported sources into
Core Wasm. Two successive self-compiled stages reproduce the same bytes. The
ordinary five-operation portable entry also compiles with both stages.

This establishes self-compilation of the current subset. The Rust compiler
remains the main implementation, with broader language, runtime and tooling
coverage. See [remaining parity work](selfhost-parity.md).

For repeated self-build retention, exception cleanup and Playground worker
lifetime checks, see [the memory audit](selfhost-memory.md).

## Run the fixed-point gate

```sh
just build-selfhost-profile target/stages/seed
just verify-selfhost-stages target/stages/run-01 target/stages/seed
just test-selfhost-stage
```

The output directory must be new. The last command builds a fresh seed and runs
the same gate in a temporary directory; it also checks that an existing capture
cannot be overwritten. It is separate from `test-selfhost` because it compiles
the compiler several times. The ordinary runtime test glob includes this gate.

The runner is [`selfhost-stage.mjs`](../runtime/selfhost-stage.mjs). It performs:

1. Load a Rust-built Walz seed through its existing Core+JSPI adapter.
2. Discover and snapshot every dependency of
   [`staged.walz`](../examples/selfhost/staged.walz).
3. Compile those exact files with the seed, producing `stage1.wasm`.
4. Instantiate stage 1 and compile the same files, producing `stage2.wasm`.
5. Independently validate both modules, require no host imports, and compare
   their bytes exactly.
6. Compile `portable.walz` with both stages, require identical validated bytes
   and the five expected Core exports.
7. Run every maintained frontend positive/negative case through the seed and
   both stages. Successful outputs must match byte for byte and execute with
   the expected result or trap. Rejections must retain the same file/code/range.

The current v10 corpus has 184 positive and 148 negative cases. The shared
Rust/bootstrap execution/rejection gate covers 107 positive and 68 negative
cases; these are different gates with different scopes.

## Host boundary

The seed uses a Rust-generated Core+JSPI transport. Self-compiled stages use a
small **Core-only GC test bridge**. JavaScript holds GC values opaquely and calls
Wasm constructors/accessors to transfer file bytes and retrieve results. Node
does file I/O and independent Wasm validation; it does not implement parsing,
linking, expansion, checking or code generation.

The bridge exports byte/file builders, byte accessors, compilation, dependency
discovery and Result inspection. Builders stay local to a request and publish
copied byte snapshots. Host-facing calls expose ordinary bytes/diagnostics;
opaque GC values remain internal to each bridge instance. There are no compiler
host imports or cross-instance GC transfers.

This bridge is not a Canonical ABI or Component adapter. Generating WIT and host
adapters with the self-compiled compiler remains future work. Existing Core+JSPI
and Component adapters for the seed still come from Rust.

The same bridge now also runs in the [Playground](playground.md#choosing-a-compiler).
Its optional `expand` export returns expanded Walz for inspection after successful
compilation; higher-level forms can remain, so this is not a native WAT printer
or a substitute for validation. The browser transfers only bytes and opaque GC
values and never loads a Rust compiler as a fallback for this mode.

## Captures and interpretation

Each capture retains:

- Compiler input bytes, original file labels and SHA-256 records.
- A separate portable-entry input snapshot.
- The complete case/probe inputs, including exact i64 expectations.
- Seed artifact and recorder hashes, engine/platform and stage output hashes.
- `stage1.wasm`, `stage2.wasm`, `portable1.wasm`, `portable2.wasm` and `report.json`.

`status: complete` requires all gates. Failures preserve the failed stage's
diagnostic or assertion. Input bytes are checked again after compilation and
against the saved snapshots. Earlier benchmark inputs and results are unchanged.

Stage elapsed times include different host transports and single cold calls;
they are diagnostic values, not compiler speed ratios. Use the
[matched-input tuning harness](selfhost-tuning.md) for performance comparisons.
Buffer/handle cleanup in the regular adapters has separate tests; it is not a
proof that every guest GC object has been collected.

## Changes that removed the blockers

- Empty result vectors mean unit, for controls and function signatures. Larger
  result vectors remain rejected rather than truncated by this single-result
  backend. The Rust typed frontend also accepts empty vectors.
- Non-final statements receive unit context; an if condition receives i32;
  final branch expressions retain their declared result type.
- Bare variable conditions are distinguished from WAT labels. Single-file
  shorthand selects the frontend, while ordinary supported WAT retains its path.
- Bare numeric literals do not resolve through numeric local-index bindings.
  Explicit `local.get` indices still refer to the original local slots.
- Bitwise/shift aliases, string-to-bytes and explicit string constructors have
  inference coverage without introducing a default integer width or conversions.
  Bare string literals still require an expected string type.
- Statement blocks in the compiler's parser/checker/stack no longer declare an
  unused value result before an unconditional return. The independent Wasm
  stack checks remain strict; general semantic-never propagation through value
  controls remains a separate parity issue.

The scalar alias table recognizes exact ASCII byte spellings. Widths, signed
and unsigned shifts, malformed spellings and ABI retirement are checked in
both adapters. This changes implementation lookup, not operator semantics.
