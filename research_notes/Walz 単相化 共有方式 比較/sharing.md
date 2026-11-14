# Generic code sharing in Go, .NET, and Java

## How does Go combine shape specialization and dictionaries?

### Takeaway

Go offers a useful middle ground: generate code for compatible shapes and carry
the remaining type-specific information in dictionaries. Its implemented shape
equivalence is deliberately stricter than equal byte size or equal GC bitmap.
Sources checked on 2026-11-15; the Go 1.18 document is historical, while the
linked `reader.go` is the current source served by go.dev.

### Cited Findings

- The Go 1.18 implementation groups arguments by underlying type or pointer
  category. It deliberately separates different built-in types, including
  integer widths, because their operators differ; builtin operations do not
  need dictionary-provided operator implementations. — [Go 1.18 implementation](https://go.googlesource.com/proposal/+/refs/heads/master/design/generics-implementation-dictionaries-go1.18.md)
- Its dictionaries contain concrete/derived type descriptors, subdictionaries,
  and interface implementation information. They are statically constructed
  and deduplicated. Static dictionary construction still requires a finite
  instantiation set: code sharing does not automatically allow polymorphic
  recursion that generates infinitely many distinct types. — [Go 1.18 implementation](https://go.googlesource.com/proposal/+/refs/heads/master/design/generics-implementation-dictionaries-go1.18.md)
- The current `Shapify` routine uses the underlying type, except that pointers
  constrained by a basic interface, excluding `NotInHeap` pointees, can use
  `*byte`. It explicitly lists more aggressive equal-size/alignment or recursive
  shaping as future work requiring type-usage tracking. — [Current Go compiler, Shapify](https://go.dev/src/cmd/compile/internal/noder/reader.go#L959)
- Current shaped calls include a runtime dictionary; type-parameter method
  expressions can load entry points from it. The source also retains concrete
  wrappers which call shaped implementations. — [Current Go compiler, callShaped and methodExpr](https://go.dev/src/cmd/compile/internal/noder/reader.go#L1450)

### Inferences

- Walz should derive a sharing key from the operations and concrete Wasm types
  required by a body, not from a general label such as “all references” or “same
  size.” This follows the conservative principle visible in Go's implementation.
- A hidden dictionary could preserve checked operations while sharing a large
  algorithm. It introduces data/lookup/possible indirect-call costs, so code
  bytes saved must be measured together with dictionaries, wrappers, validation
  time, and runtime performance. Sharing is not a guaranteed speed improvement.
- Source nominal types and emitted implementation bodies can have different
  identities, but all source constraints must be checked before sharing.

### Gaps

- No Walz-specific size or speed estimate is justified by these sources.
- The current Go source is a moving URL, not a pinned release measurement.
- The original broad GC-shape proposal is a design-space description, not proof
  that current Go groups every equal-sized/pointer-shaped type together.

## How does .NET share reference instantiations without erasing their types?

### Takeaway

.NET shares canonical reference-type code while retaining distinct runtime type
information and instantiation dictionaries. Value-type instantiations generally
need specialized native bodies; AOT moves that cost into the shipped artifact.

### Cited Findings

- The C# runtime guide describes per-value-type specialization and reuse across
  reference instantiations. Reflection can still recover actual generic type
  arguments. The guide was updated 2024-04-20. — [C# generics in the runtime](https://learn.microsoft.com/en-us/dotnet/csharp/programming-guide/generics/generics-in-the-run-time)
- The runtime's shared-generics design (authored 2019, still maintained) uses
  canonical code and per-instantiation dictionaries of type/method/field handles
  and entry points. Exact type information can require dictionary loads or
  runtime helpers, rather than constants embedded in each body. — [.NET shared generics design](https://github.com/dotnet/runtime/blob/main/docs/design/coreclr/botr/shared-generics.md)
- Current ILC architecture describes `__Canon` sharing for compatible reference
  instantiations and separate value-type bodies. It retains both canonical and
  original generic parameter identities for dependency analysis. — [NativeAOT ILC architecture](https://github.com/dotnet/runtime/blob/main/docs/design/coreclr/botr/ilc-architecture.md#compiling-generics)
- NativeAOT must generate required struct instantiations in advance, potentially
  increasing application disk size; dynamic runtimes can generate many on
  demand. This documentation was updated 2026-11-05. — [NativeAOT limitations](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/#limitations-of-native-aot-deployment)

### Inferences

- The relevant model for Walz is “shared implementation plus distinct checked
  source types,” not replacing all source types with one identity.
- A Wasm-GC backend cannot copy the CLR rule mechanically: equal-sized references
  alone do not establish compatible concrete Wasm signatures, typed struct/array
  accesses, casts, allocation instructions, ABI views, or observable type tests.
- AOT size must include support metadata. Sharing method bodies does not imply
  every specialized type descriptor or wrapper disappears.
- Canonicalization can obscure exact types from optimizers; direct specialized
  paths should remain available for hot numeric/SIMD code. This is a design
  inference, not a .NET-versus-Walz benchmark conclusion.

### Gaps

- These are implementation descriptions, not C# language requirements for every
  runtime. They do not establish a universal-canonical value-type implementation
  for all .NET targets.
- No supported claim that dictionary overhead always survives optimization, or
  that it is always optimized away, follows from these documents.

## What does Java erasure save, and what would it cost Walz?

### Takeaway

Java avoids per-type generic class duplication by erasing generic parameters to
bounds or Object, with casts and bridge methods as needed. This relies on a
reference-oriented generic type system and differs from Walz's unboxed Wasm
numeric/SIMD values and typed GC containers.

### Cited Findings

- Java SE 26 specifies erasure of a parameterized type to its generic declaration
  and a type variable to its leftmost bound. Type arguments are reference types
  or wildcards; primitive types are not direct generic arguments. — [JLS 26, type arguments and erasure](https://docs.oracle.com/javase/specs/jls/se26/html/jls-4.html#jls-4.6)
- The compiler inserts casts and bridge methods as required and does not create
  a new class for every parameterization. This Oracle tutorial targets JDK 8;
  the SE 26 specification independently confirms the erasure rule. — [Oracle type erasure tutorial](https://docs.oracle.com/javase/tutorial/java/generics/erasure.html)
- Java boxing converts primitive values to corresponding wrapper reference types,
  such as int to Integer. — [JLS 26, boxing](https://docs.oracle.com/javase/specs/jls/se26/html/jls-5.html#jls-5.1.7)

### Inferences

- Erasing Walz's i32/i64/f32/f64/v128 into one GC reference carrier would require
  boxing or another explicit representation scheme, plus recovery operations.
  It could reduce code duplication while changing allocation, locality, and
  optimization costs; it is a poor default for the language's SIMD goals.
- Erasure limited to reference-bearing containers is more plausible, but needs
  a deliberately designed common storage representation and checked wrappers.
  Existing typed containers cannot simply be reinterpreted as each other.
- Java's lack of generic bytecode duplication does not mean every generic program
  has zero casts, boxing, allocation, or native JIT code growth. Do not reuse the
  tutorial's broad “no runtime overhead” wording as a universal performance claim.

### Gaps

- Java SE 26 is the explicitly checked specification edition here, not a claim
  that it is the latest available JDK on 2026-11-15.
- HotSpot-specific JIT cloning, cast removal, and boxing-elimination rates were
  not measured. No runtime comparison with Walz was performed.
