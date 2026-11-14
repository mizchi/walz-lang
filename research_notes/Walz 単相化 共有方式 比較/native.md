# Rust C++ and Swift generic code sharing

Research date: 2026-11-15. This note separates current behavior, removed
experiments, future experiments, and recommendations for Walz.

## How does Rust control duplication and what happened to polymorphization?

### Takeaway

Rust's ordinary backend specializes concrete generic instances. Avoiding an
unused instance, caching an already-requested instance, and sharing machine code
between different instances are separate optimizations. The old optional
unused-parameter polymorphization implementation was removed; a distinct 2026
project is an experimental goal, not evidence of shipped default behavior.

### Cited Findings

- Rust emits different generic function copies for the concrete types needed by
  a program; this trades compilation work and binary size for specialization.
  The current guide no longer contains the old polymorphization section.
  — [Rust compiler development guide](https://rustc-dev-guide.rust-lang.org/backend/monomorph.html)
- Mono-item collection builds a graph rooted in codegen roots, discovers uses
  by traversing MIR, and collects concrete instantiations. Its lazy strategy
  instantiates used items; the eager strategy additionally favors stable
  incremental-compilation work. Function references/vtables/drop glue introduce
  uses beyond obvious direct source calls. Thus “only instantiate reachable
  work” is useful but not equivalent to “only instantiate directly called code.”
  — [rustc collector documentation](https://doc.rust-lang.org/stable/nightly-rustc/rustc_monomorphize/collector/index.html)
- The compiler team accepted removing the previous implementation in December
  2024. Its report described maintenance difficulties from partially
  polymorphic bodies persisting after the expected monomorphic boundary; the
  issue records the December 7, 2024 merge of PR #133883. Do not present
  `-Zpolymorphize` or the old guide's unused-parameter pass as current behavior.
  — [Compiler-team decision #810](https://github.com/rust-lang/compiler-team/issues/810)
- The accepted 2026 **experimental** goal proposes polymorphic code with
  monomorphic data, retaining layout-based groups while threading hidden
  extended vtables for trait operations, drop and type identity. It explicitly
  distinguishes the earlier unused-parameter project. This is a proposed
  experiment, not a claim that normal stable builds now use it.
  — [2026 polymorphic code generation experiment](https://github.com/rust-lang/goals/blob/main/src/2026/polymorphization.md)
- A practical source-level factoring example already exists in Rust's standard
  library: generic `std::fs::read<P: AsRef<Path>>` converts its argument to
  `&Path` and delegates its substantial body to a non-generic inner function.
  `read_to_string` and `write` use a similar boundary.
  — [Current standard-library fs source](https://doc.rust-lang.org/src/std/fs.rs.html)

### Inferences

- Walz can first retain demand-driven instance collection and its concrete
  identity cache, then factor type-independent work into ordinary helpers.
  This requires no new public generics syntax or universal erased value model.
- Eliminating codegen-irrelevant type arguments remains a valid Walz design
  candidate inspired by polymorphization research, but requires an explicit
  relevance analysis, including callees, generated signatures, type tests,
  constraints, layout, ownership and effects. A missing direct textual mention
  of a parameter is not a proof that it is irrelevant.
- Preserve a fully checked typed instance even if its eventual implementation
  is shared. This keeps source contracts distinct from codegen identity and
  avoids propagating partly substituted syntax through arbitrary backend code.

### Gaps

- These sources do not establish savings for Walz. A per-declaration inventory
  of instances, emitted body bytes and equivalent lowered bodies is needed.
- The accepted experimental Rust goal is context only; no claim is made here
  about its progress, stabilization, availability or measured benefit today.

## How does Swift share generic code while preserving optional specialization?

### Takeaway

Swift supports independently checked generic functions compiled against runtime
type information and witness operations, then optionally specializes visible
definitions for known concrete types. That is a different baseline from Rust's
ordinary concrete-instantiation backend.

### Cited Findings

- The Swift compiler author's *Compiling Swift Generics* (November 10, 2025),
  introduction and function examples, explains independent generic checking,
  runtime metadata, indirect value manipulation, and specialization when the
  definition is visible. Specialization substitutes concrete types into a
  cloned body and can eliminate abstraction overhead; `@inlinable` makes
  suitable definitions available across module boundaries.
  — [Official Swift generics implementation book](https://download.swift.org/docs/assets/generics.pdf)
- The compiler's optimization guide describes a shared generic implementation
  parameterized by runtime operations and a specialization optimization when
  concrete types and the body are visible. Its introductory treatment of boxes
  should not be generalized into a claim that every generic argument is a
  separately heap-allocated box.
  — [Swift optimization guide, Generics](https://github.com/swiftlang/swift/blob/main/docs/OptimizationTips.rst#generics)
- Type metadata's value-witness table describes move/copy/destroy operations,
  size, alignment and stride. These runtime facts support operations on values
  whose concrete layout is not embedded in the shared body.
  — [Swift type metadata ABI](https://github.com/swiftlang/swift/blob/main/docs/ABI/TypeMetadata.rst)
- SIL witness tables describe protocol-conformance dispatch. Method entries
  supply implementations; associated-type/conformance entries carry additional
  requirements. A shared generic implementation therefore preserves the
  identity of the conformance evidence rather than assuming equal layouts
  imply equal methods.
  — [Swift SIL witness tables](https://github.com/swiftlang/swift/blob/main/docs/SIL/SIL.md#witness-tables)

### Inferences

- A later Walz size-oriented mode could pass only required implementation
  evidence to a shared function while specializing hot operations. This is a
  hybrid, not necessarily global type erasure.
- For a small Wasm-GC language, the runtime surface (hidden evidence objects,
  typed indirect calls, conversion wrappers and optimizer heuristics) is a real
  implementation cost. Introduce it only after cheaper factoring/merging is
  measured, and never make it weaken source effect/ownership/opaque checks.

### Gaps

- No Swift performance number is extrapolated to Wasm-GC or Walz. In particular,
  existing Swift ABI metadata mechanisms are not directly a Wasm-GC runtime ABI.

## How do native linkers merge generic output and what transfers safely to Walz?

### Takeaway

Identical-code folding can recover binary size after instantiation, including
generic output, but it does not avoid the earlier instantiation and compilation
work. It also needs stronger equivalence and observability rules than equal
printed source or equal instruction bytes.

### Cited Findings

- LLD's ELF ICF implementation groups sections using flags, contents and
  relocation relationships. Equivalent mutually recursive functions require
  graph partition refinement because their relocations initially target
  different, but themselves equivalent, sections.
  — [LLD ICF implementation and algorithm commentary](https://github.com/llvm/llvm-project/blob/main/lld/ELF/ICF.cpp)
- Function folding can change results when a program observes function
  addresses. Safe ICF uses address-significance information to avoid such
  folds; this correctness constraint is distinct from body similarity.
  — [LLVM linker engineer discussion of safe ICF](https://lists.llvm.org/pipermail/llvm-dev/2021-March/149363.html)
- The 2026 Rust experiment notes that link-time deduplication is possible but
  requires additional analysis and happens too late to remove earlier generic
  codegen work.
  — [Rust polymorphic code generation motivation](https://github.com/rust-lang/goals/blob/main/src/2026/polymorphization.md)

### Inferences

- An initial Walz merge pass should use checked lowered IR, exact physical
  function types, alpha-normalized local/label bindings, exact referenced
  function/type identities and instruction immediates. Hashes should select
  candidates; structural equality should decide merging. Start with direct-call
  internal functions whose reference identity is not exposed.
- Equal scalar widths or both being GC references is not sufficient. Sharing
  generic code and merging GC type declarations are different operations.
  Keep source nominal/opaque identities and Wasm heap types intact unless a
  separate correctness argument establishes representation equivalence.
- For slightly different bodies, extracting the substantial common algorithm
  and retaining small typed wrappers can save code without requiring equal
  signatures. The wrapper boundary must preserve evaluation order, traps,
  effects, ownership, nullability and reference identity.
- Suggested order: measure per-declaration expansion; factor common helpers;
  merge exact checked bodies; then consider unused-argument analysis and
  selective shared generic code. Late merging is a size tool; early sharing is
  needed if the primary goal is compiler time or peak intermediate memory.

### Gaps

- This native-linker evidence is prior art, not an implementation proof for
  Walz's Core-GC emitter. The Wasm-specific signature, export, `ref.func`, table
  and identity constraints still need explicit treatment by that design.
